enum Compilation {
    Success { asm: String, stderr: String },
    Error { stderr: String },
}

#[derive(Debug, serde::Deserialize)]
struct GodboltOutputSegment {
    text: String,
}

#[derive(Debug, serde::Deserialize)]
struct GodboltOutput(Vec<GodboltOutputSegment>);

impl GodboltOutput {
    pub fn full_with_ansi_codes_stripped(&self) -> Result<String, crate::Error> {
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
}

/// Compile a given Rust source code file on Godbolt using the latest nightly compiler with
/// full optimizations (-O3)
/// Returns a multiline string with the pretty printed assembly
fn compile_rust_source(
    http: &reqwest::blocking::Client,
    source_code: &str,
) -> Result<Compilation, crate::Error> {
    let response: GodboltResponse = http
        .execute(
            http.post("https://godbolt.org/api/compiler/nightly/compile")
                .query(&[("options", "-Copt-level=3")])
                .header(reqwest::header::ACCEPT, "application/json")
                .body(source_code.to_owned())
                .build()?,
        )?
        .json()?;

    // TODO: use the extract_relevant_lines utility to strip stderr nicely
    Ok(if response.code == 0 {
        Compilation::Success {
            asm: response.asm.full_with_ansi_codes_stripped()?,
            stderr: response.stderr.full_with_ansi_codes_stripped()?,
        }
    } else {
        Compilation::Error {
            stderr: response.stderr.full_with_ansi_codes_stripped()?,
        }
    })
}

pub fn godbolt(args: &crate::Args) -> Result<(), crate::Error> {
    let (lang, text) = match compile_rust_source(args.http, crate::extract_code(&args.body)?)? {
        Compilation::Success { asm, stderr } => ("x86asm", format!("{}\n{}", stderr, asm)),
        Compilation::Error { stderr } => ("rust", stderr),
    };

    crate::reply_potentially_long_text(
        args,
        &format!("```{}\n{}", lang, text),
        "\n```",
        "Note: the output was truncated",
    )?;

    Ok(())
}

pub fn help(args: &crate::Args) -> Result<(), crate::Error> {
    serenity_framework::send_reply(
        args,
        "Compile Rust code using https://rust.godbolt.org. Full optimizations are applied.
```?godbolt ``\u{200B}`
pub fn your_function() {
    // Code
}
``\u{200B}` ```",
    )
}
