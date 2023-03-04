mod targets;
pub use targets::*;

use crate::{Context, Error};

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
    pub fn concatenate(&self) -> String {
        let mut complete_text = String::new();
        for segment in self.0.iter() {
            complete_text.push_str(&segment.text);
            complete_text.push('\n');
        }
        complete_text
    }
}

#[derive(Debug, serde::Deserialize)]
struct GodboltResponse {
    code: u8,
    // stdout: GodboltOutput,
    stderr: GodboltOutput,
    asm: GodboltOutput,
    tools: Vec<GodboltTool>,
}

#[derive(Debug, serde::Deserialize)]
struct GodboltTool {
    id: String,
    // code: u8,
    stdout: GodboltOutput,
    // stderr: GodboltOutput,
}

struct GodboltRequest<'a> {
    source_code: &'a str,
    rustc: &'a str,
    flags: &'a str,
    run_llvm_mca: bool,
}

/// Compile a given Rust source code file on Godbolt using the latest nightly compiler with
/// full optimizations (-O3)
/// Returns a multiline string with the pretty printed assembly
async fn compile_rust_source(
    http: &reqwest::Client,
    request: GodboltRequest<'_>,
) -> Result<Compilation, Error> {
    let tools = if request.run_llvm_mca {
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
            request.rustc
        ))
        .header(reqwest::header::ACCEPT, "application/json") // to make godbolt respond in JSON
        .json(&serde_json::json! { {
            "source": request.source_code,
            "options": {
                "userArguments": format!("{} --color=never", request.flags),
                "tools": tools,
                // "libraries": [{"id": "itoa", "version": "102"}],
            },
        } })
        .build()?;

    let response: GodboltResponse = http.execute(request).await?.json().await?;

    // TODO: use the extract_relevant_lines utility to strip stderr nicely
    Ok(if response.code == 0 {
        Compilation::Success {
            asm: response.asm.concatenate(),
            stderr: response.stderr.concatenate(),
            llvm_mca: match response
                .tools
                .iter()
                .find(|tool| tool.id == LLVM_MCA_TOOL_ID)
            {
                Some(llvm_mca) => Some(llvm_mca.stdout.concatenate()),
                None => None,
            },
        }
    } else {
        Compilation::Error {
            stderr: response.stderr.concatenate(),
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

async fn generic_godbolt(
    ctx: Context<'_>,
    params: poise::KeyValueArgs,
    code: poise::CodeBlock,
    mode: GodboltMode,
) -> Result<(), Error> {
    let run_llvm_mca = mode == GodboltMode::Mca;

    let (rustc, flags) = rustc_id_and_flags(ctx.data(), &params, mode).await?;

    let (lang, text);
    let mut note = String::new();

    // &code.code, &rustc, &flags, run_llvm_mca
    let godbolt_result = compile_rust_source(
        &ctx.data().http,
        GodboltRequest {
            source_code: &code.code,
            rustc: &rustc,
            flags: &flags,
            run_llvm_mca: mode == GodboltMode::Mca,
        },
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
        ctx.say(format!("``` ```{}", note)).await?;
    } else {
        super::reply_potentially_long_text(
            ctx,
            &format!("```{}\n{}", lang, text),
            &format!("\n```{}", note),
            async {
                format!(
                    "Output too large. Godbolt link: <{}>",
                    save_to_shortlink(&ctx.data().http, &code.code, &rustc, &flags, run_llvm_mca)
                        .await
                        .unwrap_or_else(|e| {
                            log::warn!("failed to generate godbolt shortlink: {}", e);
                            "failed to retrieve".to_owned()
                        }),
                )
            },
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
/// - `flags`: flags to pass to rustc invocation. Defaults to `"-Copt-level=3 --edition=2021"`
/// - `rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
#[poise::command(prefix_command, broadcast_typing, track_edits, category = "Godbolt")]
pub async fn godbolt(
    ctx: Context<'_>,
    params: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    generic_godbolt(ctx, params, code, GodboltMode::Asm).await
}

fn strip_llvm_mca_result(text: &str) -> &str {
    text[..text.find("Instruction Info").unwrap_or(text.len())].trim()
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
/// - `flags`: flags to pass to rustc invocation. Defaults to `"-Copt-level=3 --edition=2021"`
/// - `rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
#[poise::command(prefix_command, broadcast_typing, track_edits, category = "Godbolt")]
pub async fn mca(
    ctx: Context<'_>,
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
/// - `flags`: flags to pass to rustc invocation. Defaults to `"-Copt-level=3 --edition=2021"`
/// - `rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
#[poise::command(prefix_command, broadcast_typing, track_edits, category = "Godbolt")]
pub async fn llvmir(
    ctx: Context<'_>,
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
/// - `flags`: flags to pass to rustc invocation. Defaults to `"-Copt-level=3 --edition=2021"`
/// - `rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
#[poise::command(
    prefix_command,
    broadcast_typing,
    track_edits,
    hide_in_help,
    category = "Godbolt"
)]
pub async fn asmdiff(
    ctx: Context<'_>,
    params: poise::KeyValueArgs,
    code1: poise::CodeBlock,
    code2: poise::CodeBlock,
) -> Result<(), Error> {
    let (rustc, flags) = rustc_id_and_flags(ctx.data(), &params, GodboltMode::Asm).await?;

    let req1 = GodboltRequest {
        source_code: &code1.code,
        rustc: &rustc,
        flags: &flags,
        run_llvm_mca: false,
    };
    let req2 = GodboltRequest {
        source_code: &code2.code,
        ..req1
    };
    let (asm1, asm2) = tokio::try_join!(
        compile_rust_source(&ctx.data().http, req1),
        compile_rust_source(&ctx.data().http, req2),
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
                .args(["diff", "--no-index"])
                .arg(&path1)
                .arg(&path2)
                .output()
                .await?
                .stdout;

            super::reply_potentially_long_text(
                ctx,
                &format!("```diff\n{}", String::from_utf8_lossy(&diff)),
                "```",
                async { String::from("(output was truncated)") },
            )
            .await?;
        }
        Err(stderr) => {
            super::reply_potentially_long_text(
                ctx,
                &format!("```rust\n{}", stderr),
                "```",
                async { String::from("(output was truncated)") },
            )
            .await?;
        }
    }

    Ok(())
}
