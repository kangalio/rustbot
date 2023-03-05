mod targets;
pub use targets::*;

use crate::{Context, Error};

const LLVM_MCA_TOOL_ID: &str = "llvm-mcatrunk";

struct Compilation {
    output: String,
    stderr: String,
    success: bool,
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
    request: &GodboltRequest<'_>,
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

    let http_request = http
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

    let response: GodboltResponse = http.execute(http_request).await?.json().await?;

    // TODO: use the extract_relevant_lines utility to strip stderr nicely
    Ok(Compilation {
        output: if request.run_llvm_mca {
            let text = response
                .tools
                .iter()
                .find(|tool| tool.id == LLVM_MCA_TOOL_ID)
                .map(|llvm_mca| llvm_mca.stdout.concatenate())
                .ok_or("No llvm-mca result was sent by Godbolt")?;
            // Strip junk
            text[..text.find("Instruction Info").unwrap_or(text.len())]
                .trim()
                .to_string()
        } else {
            response.asm.concatenate()
        },
        stderr: response.stderr.concatenate(),
        success: response.code == 0,
    })
}

async fn save_to_shortlink(http: &reqwest::Client, req: &GodboltRequest<'_>) -> String {
    #[derive(serde::Deserialize)]
    struct GodboltShortenerResponse {
        url: String,
    }

    let tools = if req.run_llvm_mca {
        serde_json::json! {
            [{"id": LLVM_MCA_TOOL_ID}]
        }
    } else {
        serde_json::json! {
            []
        }
    };

    let request = http
        .post("https://godbolt.org/api/shortener")
        .json(&serde_json::json! { {
            "sessions": [{
                "language": "rust",
                "source": req.source_code,
                "compilers": [{
                    "id": req.rustc,
                    "options": req.flags,
                    "tools": tools,
                }],
            }]
        } });

    // Try block substitute
    let url = async move {
        Ok::<_, crate::Error>(
            request
                .send()
                .await?
                .json::<GodboltShortenerResponse>()
                .await?
                .url,
        )
    };
    url.await.unwrap_or_else(|e| {
        log::warn!("failed to generate godbolt shortlink: {}", e);
        "failed to retrieve".to_owned()
    })
}

#[derive(PartialEq, Clone, Copy)]
enum GodboltMode {
    Asm,
    LlvmIr,
    Mca,
}

async fn respond_codeblock(
    ctx: Context<'_>,
    codeblock_lang: &str,
    text: &str,
    note: &str,
    godbolt_request: &GodboltRequest<'_>,
) -> Result<(), Error> {
    ctx.say(
        crate::trim_text(
            &format!("```{}\n{}", codeblock_lang, text),
            &format!("\n```{}", note),
            async {
                format!(
                    "Output too large. Godbolt link: <{}>",
                    save_to_shortlink(&ctx.data().http, &godbolt_request).await,
                )
            },
        )
        .await,
    )
    .await?;
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
    let (rustc, flags) = rustc_id_and_flags(ctx.data(), &params).await?;
    let godbolt_request = GodboltRequest {
        source_code: &code.code,
        rustc: &rustc,
        flags: &flags,
        run_llvm_mca: false,
    };
    let godbolt_result = compile_rust_source(&ctx.data().http, &godbolt_request).await?;

    let text = crate::merge_output_and_errors(&godbolt_result.output, &godbolt_result.stderr);
    let note = if code.code.contains("pub fn") {
        "Note: only public functions (`pub fn`) are shown"
    } else {
        ""
    };
    let codeblock_lang = if godbolt_result.success {
        "x86asm"
    } else {
        "rust"
    };
    respond_codeblock(ctx, codeblock_lang, &text, note, &godbolt_request).await?;

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
/// - `flags`: flags to pass to rustc invocation. Defaults to `"-Copt-level=3 --edition=2021"`
/// - `rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
#[poise::command(prefix_command, broadcast_typing, track_edits, category = "Godbolt")]
pub async fn mca(
    ctx: Context<'_>,
    params: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let (rustc, flags) = rustc_id_and_flags(ctx.data(), &params).await?;
    let godbolt_request = GodboltRequest {
        source_code: &code.code,
        rustc: &rustc,
        flags: &flags,
        run_llvm_mca: true,
    };

    let godbolt_result = compile_rust_source(&ctx.data().http, &godbolt_request).await?;

    let text = crate::merge_output_and_errors(&godbolt_result.output, &godbolt_result.stderr);
    let note = if code.code.contains("pub fn") {
        ""
    } else {
        "Note: only public functions (`pub fn`) are shown"
    };
    respond_codeblock(ctx, "rust", &text, note, &godbolt_request).await?;

    Ok(())
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
    let (rustc, flags) = rustc_id_and_flags(ctx.data(), &params).await?;
    let godbolt_request = GodboltRequest {
        source_code: &code.code,
        rustc: &rustc,
        flags: &(flags + " --emit=llvm-ir -Cdebuginfo=0"),
        run_llvm_mca: false,
    };
    let godbolt_result = compile_rust_source(&ctx.data().http, &godbolt_request).await?;

    let text = crate::merge_output_and_errors(&godbolt_result.output, &godbolt_result.stderr);
    let codeblock_lang = if godbolt_result.success {
        "llvm"
    } else {
        "rust"
    };
    let note = if code.code.contains("pub fn") {
        ""
    } else {
        "Note: only public functions (`pub fn`) are shown"
    };
    respond_codeblock(ctx, codeblock_lang, &text, &note, &godbolt_request).await?;

    Ok(())
}
