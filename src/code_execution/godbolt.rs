use crate::{Error, PrefixContext};

const LLVM_MCA_TOOL_ID: &str = "llvm-mcatrunk";

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

// Transforms human readable rustc version (e.g. "1.34.1") into compiler id on godbolt (e.g. "r1341")
// Full list of version<->id can be obtained at https://godbolt.org/api/compilers/rust
// Ideally we'd also check that the version exists, and give a nice error message if not, but eh.
fn translate_rustc_version(version: &str) -> Result<std::borrow::Cow<'_, str>, Error> {
    if ["nightly", "beta"].contains(&version) {
        return Ok(version.into());
    }
    // very crude sanity checking
    if !version.chars().all(|c| c.is_digit(10) || c == '.') {
        return Err(
            "the `rustc` argument should be a version specifier. E.g. `nightly` `beta` or `1.45.2`"
                .into(),
        );
    }
    Ok(format!("r{}", version.replace(".", "")).into())
}

/// Compile a given Rust source code file on Godbolt using the latest nightly compiler with
/// full optimizations (-O3)
/// Returns a multiline string with the pretty printed assembly
async fn compile_rust_source(
    http: &reqwest::Client,
    source_code: &str,
    rustc: &str,
    flags: &str,
    run_llvm_mca: bool,
) -> Result<Compilation, Error> {
    let rustc = translate_rustc_version(rustc)?;

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
        .header(reqwest::header::ACCEPT, "application/json") // to make godbolt respond in JSON
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

async fn generic_godbolt(
    ctx: PrefixContext<'_>,
    params: poise::KeyValueArgs,
    code: poise::CodeBlock,
    mca_mode: bool,
) -> Result<(), Error> {
    let rustc = params.get("rustc").unwrap_or(&"nightly");
    let flags = params
        .get("flags")
        .unwrap_or(&"-Copt-level=3 --edition=2018");
    let (lang, text, note) =
        match compile_rust_source(&ctx.data.http, &code.code, rustc, flags, mca_mode).await? {
            Compilation::Success {
                asm,
                stderr,
                llvm_mca,
            } => (
                if mca_mode { "rust" } else { "x86asm" },
                if mca_mode {
                    strip_llvm_mca_result(
                        &llvm_mca.ok_or("No llvm-mca result was sent by Godbolt")?,
                    )
                    .to_owned()
                } else {
                    asm
                },
                (!stderr.is_empty()).then(|| "Note: compilation produced warnings\n"),
            ),
            Compilation::Error { stderr } => ("rust", stderr, None),
        };

    let note = note.unwrap_or("");

    if text.trim().is_empty() {
        poise::say_prefix_reply(ctx, format!("``` ```{}", note)).await?;
    } else {
        super::reply_potentially_long_text(
            ctx,
            &format!("```{}\n{}", lang, text),
            &format!("\n```{}", note),
            &format!(
                "Output too large. Godbolt link: <{}>",
                save_to_shortlink(&ctx.data.http, &code.code, rustc, flags, mca_mode).await?,
            ),
        )
        .await?;
    }

    Ok(())
}

/// View assembly using Godbolt
///
/// Compile Rust code using https://rust.godbolt.org. Full optimizations are applied unless \
/// overriden.
/// ```
/// ?godbolt ``​`
/// pub fn your_function() {
///     // Code
/// }
/// ``​`
/// ```
/// Optional arguments:
/// - `flags`: flags to pass to rustc invocation. Defaults to `"-Copt-level=3 --edition=2018"`
/// - `rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
#[poise::command(broadcast_typing, track_edits)]
pub async fn godbolt(
    ctx: PrefixContext<'_>,
    params: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    generic_godbolt(ctx, params, code, false).await
}

fn strip_llvm_mca_result(text: &str) -> &str {
    text[..text.find("Instruction Info").unwrap_or(text.len())].trim()
}

/// Run performance analysis using llvm-mca
///
/// Runs the performance analysis tool llvm-mca using https://rust.godbolt.org. Full optimizations \
/// are applied unless overriden.
/// ```
/// ?mca ``​`
/// pub fn your_function() {
///     // Code
/// }
/// ``​`
/// ```
/// Optional arguments:
/// - `flags`: flags to pass to rustc invocation. Defaults to `"-Copt-level=3 --edition=2018"`
/// - `rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
#[poise::command(broadcast_typing, track_edits)]
pub async fn mca(
    ctx: PrefixContext<'_>,
    params: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    generic_godbolt(ctx, params, code, true).await
}
