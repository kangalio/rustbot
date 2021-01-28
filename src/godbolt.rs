pub enum Compilation {
    Success { asm: String },
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
            complete_text.push_str("\n");
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
pub fn compile_rust_source(
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

    dbg!(&response);

    Ok(if response.code == 0 {
        Compilation::Success {
            asm: response.asm.full_with_ansi_codes_stripped()?,
        }
    } else {
        Compilation::Error {
            stderr: response.stderr.full_with_ansi_codes_stripped()?,
        }
    })
}
