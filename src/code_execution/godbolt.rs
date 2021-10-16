use crate::{Data, Error, PrefixContext};
use chrono::{TimeZone, Utc};
use sqlx::{pool::PoolConnection, Connection, Sqlite};
use std::{cmp::Reverse, collections::HashMap, env, time::Duration};

const LLVM_MCA_TOOL_ID: &str = "llvm-mcatrunk";
const GODBOLT_TARGETS_URL: &str = "https://godbolt.org/api/compilers/rust";
const ACCEPT_JSON: &str = "application/json";

enum Compilation {
    Success {
        asm: String,
        stderr: String,
        llvm_mca: Option<String>,
    },
    Error {
        stderr: String,
    },
}

#[derive(Debug, serde::Deserialize)]
struct GodboltOutputSegment {
    text: String,
}

#[derive(Debug, serde::Deserialize)]
struct GodboltOutput(Vec<GodboltOutputSegment>);

impl GodboltOutput {
    pub fn full_with_ansi_codes_stripped(&self) -> Result<String, Error> {
        let mut complete_text = String::new();
        for segment in self.0.iter() {
            complete_text.push_str(&segment.text);
            complete_text.push('\n');
        }
        Ok(String::from_utf8(strip_ansi_escapes::strip(
            complete_text.trim(),
        )?)?)
    }
}

#[derive(Debug, serde::Deserialize)]
struct GodboltResponse {
    code: u8,
    stdout: GodboltOutput,
    stderr: GodboltOutput,
    asm: GodboltOutput,
    tools: Vec<GodboltTool>,
}

#[derive(Debug, serde::Deserialize)]
struct GodboltTool {
    id: String,
    code: u8,
    stdout: GodboltOutput,
    stderr: GodboltOutput,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GodboltTarget {
    id: String,
    name: String,
    lang: String,
    compiler_type: String,
    semver: String,
    instruction_set: String,
}

impl GodboltTarget {
    fn clean_request_data(&mut self) {
        // Some semvers get weird characters like `()` in them or spaces, we strip that out here
        self.semver = self
            .semver
            .chars()
            .filter(|char| char.is_alphanumeric() || matches!(char, '.' | '-' | '_'))
            .map(|char| char.to_ascii_lowercase())
            .collect();
    }
}

async fn update_godbolt_targets(
    conn: &mut PoolConnection<Sqlite>,
    data: &Data,
) -> Result<(), Error> {
    // Fetch the last time we updated the targets list, this will be null/none if we've never
    // done it before
    let last_update_time = sqlx::query!("SELECT last_update FROM last_godbolt_update")
        .fetch_optional(&mut *conn)
        .await?;

    let needs_update = if let Some(last_update_time) = last_update_time {
        // Convert the stored timestamp into a utc date time
        let last_update_time = Utc.timestamp_nanos(last_update_time.last_update);

        // Get the time to wait between each update of the godbolt targets list
        let update_period = chrono::Duration::from_std(
            env::var("GODBOLT_UPDATE_DURATION")
                .ok()
                .and_then(|duration| duration.parse::<u64>().ok())
                .map(Duration::from_secs)
                // Currently set to 12 hours
                .unwrap_or_else(|| Duration::from_secs(60 * 60 * 12)),
        )?;

        let time_since_update = Utc::now().signed_duration_since(last_update_time);
        let needs_update = time_since_update >= update_period;
        if needs_update {
            log::info!(
                "godbolt targets were last updated {:#?} ago, updating them",
                time_since_update,
            );
        }

        needs_update
    } else {
        log::info!("godbolt targets haven't yet been updated, fetching them");

        true
    };

    // If we should perform an update then do so
    if needs_update {
        let request = data
            .http
            .get(GODBOLT_TARGETS_URL)
            .header(reqwest::header::ACCEPT, ACCEPT_JSON)
            .build()?;

        let mut targets: Vec<GodboltTarget> = data.http.execute(request).await?.json().await?;
        log::info!("got {} godbolt targets", targets.len());

        // Clean up the data we've gotten from the request
        for target in &mut targets {
            target.clean_request_data();
        }

        // Run the target updates within a transaction so that if things go wrong we don't
        // end up with an empty targets list
        conn.transaction::<_, (), Error>(|conn| {
            Box::pin(async move {
                // Remove all old values from the targets list, this is probably overly-cautious
                // but it at least ensures that we never have incorrect targets within the db
                sqlx::query!("DELETE FROM godbolt_targets")
                    .execute(&mut *conn)
                    .await?;

                // Insert all of our newly fetched targets into the db
                for target in targets {
                    // Some versions have a leading `rustc ` (`rustc beta`, `rustc 1.40.0`, etc.),
                    // so we strip that here so we only have to do it once
                    let semver = target
                        .semver
                        .strip_prefix("rustc ")
                        .unwrap_or(&*target.semver);

                    sqlx::query!(
                        "INSERT INTO godbolt_targets (
                            id,
                            name,
                            lang,
                            compiler_type,
                            semver,
                            instruction_set
                         )
                         VALUES ($1, $2, $3, $4, $5, $6)",
                        target.id,
                        target.name,
                        target.lang,
                        target.compiler_type,
                        semver,
                        target.instruction_set,
                    )
                    .execute(&mut *conn)
                    .await?;
                }

                // Get the current utc timestamp
                let current_time = Utc::now().timestamp_nanos();

                // Set the last godbolt update time to now
                sqlx::query!(
                    "INSERT INTO last_godbolt_update (id, last_update)
                     VALUES (0, $1)
                     ON CONFLICT(id) DO UPDATE SET last_update = $1
                     WHERE id = 0",
                    current_time,
                )
                .execute(&mut *conn)
                .await?;

                Ok(())
            })
        })
        .await?;

        log::info!("finished updating godbolt targets list");
    }

    Ok(())
}

async fn fetch_godbolt_targets(data: &Data) -> Result<HashMap<String, String>, Error> {
    let mut conn = data.database.acquire().await?;

    // If we encounter an error while updating the targets list, just log it
    if let Err(error) = update_godbolt_targets(&mut conn, data).await {
        log::error!("failed to update godbolt targets list: {:?}", error);
    }

    log::info!("fetching godbolt targets");
    let query = sqlx::query!("SELECT id, semver FROM godbolt_targets")
        .fetch_all(&mut conn)
        .await?;

    let targets: HashMap<_, _> = query
        .into_iter()
        .map(|target| (target.semver, target.id))
        .collect();

    log::debug!("fetched {} godbolt targets", targets.len());
    Ok(targets)
}

// Transforms human readable rustc version (e.g. "1.34.1") into compiler id on godbolt (e.g. "r1341")
// Full list of version<->id can be obtained at https://godbolt.org/api/compilers/rust
// Ideally we'd also check that the version exists, and give a nice error message if not, but eh.
fn translate_rustc_version<'a>(
    version: &str,
    targets: &'a HashMap<String, String>,
) -> Result<&'a str, Error> {
    if let Some(godbolt_id) = targets.get(version.trim()) {
        Ok(godbolt_id)
    } else {
        Err(
            "the `rustc` argument should be a version specifier like `nightly` `beta` or `1.45.2`. \
             Run ?godbolt-targets for a full list"
                .into(),
        )
    }
}

/// Compile a given Rust source code file on Godbolt using the latest nightly compiler with
/// full optimizations (-O3)
/// Returns a multiline string with the pretty printed assembly
async fn compile_rust_source(
    http: &reqwest::Client,
    targets: &HashMap<String, String>,
    source_code: &str,
    rustc: &str,
    flags: &str,
    run_llvm_mca: bool,
) -> Result<Compilation, Error> {
    let rustc = translate_rustc_version(rustc, targets)?;

    let tools = if run_llvm_mca {
        serde_json::json! {
            [{"id": LLVM_MCA_TOOL_ID}]
        }
    } else {
        serde_json::json! {
            []
        }
    };

    let request = http
        .post(&format!(
            "https://godbolt.org/api/compiler/{}/compile",
            rustc
        ))
        .header(reqwest::header::ACCEPT, ACCEPT_JSON) // to make godbolt respond in JSON
        .json(&serde_json::json! { {
            "source": source_code,
            "options": {
                "userArguments": flags,
                "tools": tools,
            },
        } })
        .build()?;

    let response: GodboltResponse = http.execute(request).await?.json().await?;

    // TODO: use the extract_relevant_lines utility to strip stderr nicely
    Ok(if response.code == 0 {
        Compilation::Success {
            asm: response.asm.full_with_ansi_codes_stripped()?,
            stderr: response.stderr.full_with_ansi_codes_stripped()?,
            llvm_mca: match response
                .tools
                .iter()
                .find(|tool| tool.id == LLVM_MCA_TOOL_ID)
            {
                Some(llvm_mca) => Some(llvm_mca.stdout.full_with_ansi_codes_stripped()?),
                None => None,
            },
        }
    } else {
        Compilation::Error {
            stderr: response.stderr.full_with_ansi_codes_stripped()?,
        }
    })
}

async fn save_to_shortlink(
    http: &reqwest::Client,
    code: &str,
    rustc: &str,
    flags: &str,
    run_llvm_mca: bool,
) -> Result<String, Error> {
    #[derive(serde::Deserialize)]
    struct GodboltShortenerResponse {
        url: String,
    }

    let tools = if run_llvm_mca {
        serde_json::json! {
            [{"id": LLVM_MCA_TOOL_ID}]
        }
    } else {
        serde_json::json! {
            []
        }
    };

    let response = http
        .post("https://godbolt.org/api/shortener")
        .json(&serde_json::json! { {
            "sessions": [{
                "language": "rust",
                "source": code,
                "compilers": [{
                    "id": rustc,
                    "options": flags,
                    "tools": tools,
                }],
            }]
        } })
        .send()
        .await?;

    Ok(response.json::<GodboltShortenerResponse>().await?.url)
}

#[derive(PartialEq, Clone, Copy)]
enum GodboltMode {
    Asm,
    LlvmIr,
    Mca,
}

fn rustc_version_and_flags(params: &poise::KeyValueArgs, mode: GodboltMode) -> (&str, String) {
    let rustc = params.get("rustc").unwrap_or("nightly");
    let mut flags = params
        .get("flags")
        .unwrap_or("-Copt-level=3 --edition=2018")
        .to_owned();

    if mode == GodboltMode::LlvmIr {
        flags += " --emit=llvm-ir -Cdebuginfo=0";
    }

    (rustc, flags)
}

async fn generic_godbolt(
    ctx: PrefixContext<'_>,
    params: poise::KeyValueArgs,
    code: poise::CodeBlock,
    mode: GodboltMode,
) -> Result<(), Error> {
    let run_llvm_mca = mode == GodboltMode::Mca;

    let (rustc, flags) = rustc_version_and_flags(&params, mode);

    let (lang, text);
    let mut note = String::new();

    let targets = fetch_godbolt_targets(ctx.data).await?;
    let godbolt_result = compile_rust_source(
        &ctx.data.http,
        &targets,
        &code.code,
        rustc,
        &flags,
        run_llvm_mca,
    )
    .await?;

    match godbolt_result {
        Compilation::Success {
            asm,
            stderr,
            llvm_mca,
        } => {
            lang = match mode {
                GodboltMode::Asm => "x86asm",
                GodboltMode::Mca => "rust",
                GodboltMode::LlvmIr => "llvm",
            };
            text = match mode {
                GodboltMode::Mca => {
                    let llvm_mca = llvm_mca.ok_or("No llvm-mca result was sent by Godbolt")?;
                    strip_llvm_mca_result(&llvm_mca).to_owned()
                }
                GodboltMode::Asm | GodboltMode::LlvmIr => asm,
            };
            if !stderr.is_empty() {
                note += "Note: compilation produced warnings\n";
            }
        }
        Compilation::Error { stderr } => {
            lang = "rust";
            text = stderr;
        }
    };

    if !code.code.contains("pub fn") {
        note += "Note: only public functions (`pub fn`) are shown\n";
    }

    if text.trim().is_empty() {
        poise::say_reply(ctx.into(), format!("``` ```{}", note)).await?;
    } else {
        super::reply_potentially_long_text(
            ctx,
            &format!("```{}\n{}", lang, text),
            &format!("\n```{}", note),
            &format!(
                "Output too large. Godbolt link: <{}>",
                save_to_shortlink(&ctx.data.http, &code.code, rustc, &flags, run_llvm_mca).await?,
            ),
        )
        .await?;
    }

    Ok(())
}

/// View assembly using Godbolt
///
/// Compile Rust code using <https://rust.godbolt.org>. Full optimizations are applied unless \
/// overriden.
/// ```
/// ?godbolt flags={} rustc={} ``​`
/// pub fn your_function() {
///     // Code
/// }
/// ``​`
/// ```
/// Optional arguments:
/// - `flags`: flags to pass to rustc invocation. Defaults to `"-Copt-level=3 --edition=2018"`
/// - `rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
#[poise::command(prefix_command, broadcast_typing, track_edits)]
pub async fn godbolt(
    ctx: PrefixContext<'_>,
    params: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    generic_godbolt(ctx, params, code, GodboltMode::Asm).await
}

fn strip_llvm_mca_result(text: &str) -> &str {
    text[..text.find("Instruction Info").unwrap_or_else(|| text.len())].trim()
}

/// Used to rank godbolt compiler versions for listing them out
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SemverRanking<'a> {
    Beta,
    Nightly,
    Compiler(&'a str),
    Semver(Reverse<(u16, u16, u16)>),
}

impl<'a> From<&'a str> for SemverRanking<'a> {
    fn from(semver: &'a str) -> Self {
        match semver {
            "beta" => Self::Beta,
            "nightly" => Self::Nightly,

            semver => {
                // Rustc versions are received in a `X.X.X` form, so we parse out
                // the major/minor/patch versions and then order them in *reverse*
                // order based on their version triple, this means that the most
                // recent (read: higher) versions will be at the top of the list
                let mut version_triple = semver.splitn(3, '.');
                let version_triple = version_triple
                    .next()
                    .zip(version_triple.next())
                    .zip(version_triple.next())
                    .and_then(|((major, minor), patch)| {
                        Some((
                            major.parse().ok()?,
                            minor.parse().ok()?,
                            patch.parse().ok()?,
                        ))
                    });

                // If we successfully parsed out a semver tuple, return it
                if let Some((major, minor, patch)) = version_triple {
                    Self::Semver(Reverse((major, minor, patch)))

                // Anything that doesn't fit the `X.X.X` format we treat as an alternative
                // compiler, we list these after beta & nightly but before the many canonical
                // rustc versions
                } else {
                    Self::Compiler(semver)
                }
            }
        }
    }
}

/// Lists all available godbolt rustc targets
#[poise::command(prefix_command, broadcast_typing)]
pub async fn godbolt_targets(ctx: PrefixContext<'_>) -> Result<(), Error> {
    let mut conn = ctx.data.database.acquire().await?;

    // Attempt to update the godbolt targets list, logging errors if they occur
    if let Err(error) = update_godbolt_targets(&mut conn, ctx.data).await {
        log::error!("failed to update godbolt targets list: {:?}", error);
    }

    let mut targets = sqlx::query!("SELECT name, semver, instruction_set FROM godbolt_targets")
        .fetch_all(&mut conn)
        .await?;

    targets.sort_unstable_by(|lhs, rhs| {
        SemverRanking::from(&*lhs.semver).cmp(&SemverRanking::from(&*rhs.semver))
    });

    poise::send_reply(ctx.into(), |msg| {
        msg.embed(|embed| {
            embed
                .title("Godbolt Targets")
                .fields(targets.into_iter().map(|target| {
                    (
                        target.semver,
                        format!("{} (runs on {})", target.name, target.instruction_set),
                        true,
                    )
                }))
        })
    })
    .await?;

    Ok(())
}

/// Run performance analysis using llvm-mca
///
/// Run the performance analysis tool llvm-mca using <https://rust.godbolt.org>. Full optimizations \
/// are applied unless overriden.
/// ```
/// ?mca flags={} rustc={} ``​`
/// pub fn your_function() {
///     // Code
/// }
/// ``​`
/// ```
/// Optional arguments:
/// - `flags`: flags to pass to rustc invocation. Defaults to `"-Copt-level=3 --edition=2018"`
/// - `rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
#[poise::command(prefix_command, broadcast_typing, track_edits)]
pub async fn mca(
    ctx: PrefixContext<'_>,
    params: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    generic_godbolt(ctx, params, code, GodboltMode::Mca).await
}

/// View LLVM IR using Godbolt
///
/// Compile Rust code using <https://rust.godbolt.org> and emits LLVM IR. Full optimizations \
/// are applied unless overriden.
///
/// Equivalent to ?godbolt but with extra flags `--emit=llvm-ir -Cdebuginfo=0`.
/// ```
/// ?llvmir flags={} rustc={} ``​`
/// pub fn your_function() {
///     // Code
/// }
/// ``​`
/// ```
/// Optional arguments:
/// - `flags`: flags to pass to rustc invocation. Defaults to `"-Copt-level=3 --edition=2018"`
/// - `rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
#[poise::command(prefix_command, broadcast_typing, track_edits)]
pub async fn llvmir(
    ctx: PrefixContext<'_>,
    params: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    generic_godbolt(ctx, params, code, GodboltMode::LlvmIr).await
}

// TODO: adjust doc
/// View difference between assembled functions
///
/// Compiles two Rust code snippets using <https://rust.godbolt.org> and diffs them. Full optimizations \
/// are applied unless overriden.
/// ```
/// ?asmdiff flags={} rustc={} ``​`
/// pub fn foo(x: u32) -> u32 {
///     x
/// }
/// ``​` ``​`
/// pub fn foo(x: u64) -> u64 {
///     x
/// }
/// ``​`
/// ```
/// Optional arguments:
/// - `flags`: flags to pass to rustc invocation. Defaults to `"-Copt-level=3 --edition=2018"`
/// - `rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
#[poise::command(prefix_command, broadcast_typing, track_edits, hide_in_help)]
pub async fn asmdiff(
    ctx: PrefixContext<'_>,
    params: poise::KeyValueArgs,
    code1: poise::CodeBlock,
    code2: poise::CodeBlock,
) -> Result<(), Error> {
    let (rustc, flags) = rustc_version_and_flags(&params, GodboltMode::Asm);

    let targets = fetch_godbolt_targets(ctx.data).await?;
    let (asm1, asm2) = tokio::try_join!(
        compile_rust_source(&ctx.data.http, &targets, &code1.code, rustc, &flags, false),
        compile_rust_source(&ctx.data.http, &targets, &code2.code, rustc, &flags, false),
    )?;
    let result = match (asm1, asm2) {
        (Compilation::Success { asm: a, .. }, Compilation::Success { asm: b, .. }) => Ok((a, b)),
        (Compilation::Error { stderr }, _) => Err(stderr),
        (_, Compilation::Error { stderr }) => Err(stderr),
    };

    match result {
        Ok((asm1, asm2)) => {
            let mut path1 = std::env::temp_dir();
            path1.push("a");
            tokio::fs::write(&path1, asm1).await?;

            let mut path2 = std::env::temp_dir();
            path2.push("b");
            tokio::fs::write(&path2, asm2).await?;

            let diff = tokio::process::Command::new("git")
                .args(&["diff", "--no-index"])
                .arg(&path1)
                .arg(&path2)
                .output()
                .await?
                .stdout;

            super::reply_potentially_long_text(
                ctx,
                &format!("```diff\n{}", String::from_utf8_lossy(&diff)),
                "```",
                "(output was truncated)",
            )
            .await?;
        }
        Err(stderr) => {
            super::reply_potentially_long_text(
                ctx,
                &format!("```rust\n{}", stderr),
                "```",
                "(output was truncated)",
            )
            .await?;
        }
    }

    Ok(())
}
