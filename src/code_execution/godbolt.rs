use crate::{Context, Error};

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
fn compile_rust_source(
    http: &reqwest::blocking::Client,
    source_code: &str,
    rustc: &str,
    flags: &str,
) -> Result<Compilation, Error> {
    let rustc = translate_rustc_version(rustc)?;
    let response: GodboltResponse = http
        .execute(
            http.post(&format!(
                "https://godbolt.org/api/compiler/{}/compile",
                rustc
            ))
            .query(&[("options", flags)])
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

pub fn godbolt(ctx: Context<'_>, args: &str) -> Result<(), Error> {
    let (params, code) = poise::parse_args!(args => (poise::KeyValueArgs), (poise::CodeBlock))?;

    let rustc = params.get("rustc").unwrap_or(&"nightly");
    let flags = params
        .get("flags")
        .unwrap_or(&"-Copt-level=3 --edition=2018");
    println!("r f = {:?} {:?}", rustc, flags);
    let (lang, text, note) = match compile_rust_source(&ctx.data.http, &code.code, rustc, flags)? {
        Compilation::Success { asm, stderr } => (
            "x86asm",
            asm,
            (!stderr.is_empty()).then(|| "Note: compilation produced warnings\n"),
        ),
        Compilation::Error { stderr } => ("rust", stderr, None),
    };

    super::reply_potentially_long_text(
        ctx,
        &format!("```{}\n{}", lang, text),
        &format!("\n```{}", note.unwrap_or("")),
        "Note: the output was truncated",
    )?;

    Ok(())
}
