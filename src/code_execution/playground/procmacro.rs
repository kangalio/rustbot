use super::{api::*, util::*};
use crate::{Error, PrefixContext};

/// Compile and use a procedural macro
#[poise::command(track_edits, broadcast_typing, explanation_fn = "procmacro_help")]
pub async fn procmacro(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    macro_code: poise::CodeBlock,
    usage_code: poise::CodeBlock,
) -> Result<(), Error> {
    let macro_code = macro_code.code;
    let usage_code = maybe_wrap(&usage_code.code, ResultHandling::None);

    let (flags, flag_parse_errors) = parse_flags(&flags);

    let generated_code = format!(
        "{}{}{}{}{}{}{}",
        r#"const MACRO_CODE: &str = r#####""#,
        macro_code,
        "\"",
        r#"#####;
const USAGE_CODE: &str = r#####""#,
        usage_code,
        "\"",
        r#"#####;
pub fn cmd_run(cmd: &str) {
    let status = std::process::Command::new("/bin/sh")
        .args(&["-c", cmd])
        .status()
        .unwrap();
    if !status.success() {
        std::process::exit(-1);
    }
}

pub fn cmd_stdout(cmd: &str) -> String {
    let output = std::process::Command::new("/bin/sh")
        .args(&["-c", cmd])
        .output()
        .unwrap();
    String::from_utf8(output.stdout).unwrap()
}

fn main() -> std::io::Result<()> {
    use std::io::Write as _;
    std::env::set_current_dir(cmd_stdout("mktemp -d").trim())?;
    cmd_run("cargo init -q --name procmacro --lib");
    std::fs::write("src/lib.rs", MACRO_CODE)?;
    std::fs::write("src/main.rs", USAGE_CODE)?;
    std::fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open("Cargo.toml")?
        .write_all(b"[lib]\nproc-macro = true")?;
    cmd_run("cargo c -q --bin procmacro");
    Ok(())
}"#
    );

    let mut result: PlayResult = ctx
        .data
        .http
        .post("https://play.rust-lang.org/execute")
        .json(&PlaygroundRequest {
            code: &generated_code,
            // These flags only apply to the glue code
            channel: Channel::Stable,
            crate_type: CrateType::Binary,
            edition: Edition::E2018,
            mode: Mode::Debug,
            tests: false,
        })
        .send()
        .await?
        .json()
        .await?;

    // funky
    result.stderr = format_play_eval_stderr(
        &format_play_eval_stderr(&result.stderr, flags.warn),
        flags.warn,
    );

    send_reply(ctx, result, &generated_code, &flags, &flag_parse_errors).await
}

pub fn procmacro_help() -> String {
    generic_help(GenericHelp {
        command: "procmacro",
        desc: "Compile and use a procedural macro by providing two snippets: one for the \
        proc-macro code, and one for the usage code which can refer to the proc-macro crate as \
        `procmacro`",
        mode_and_channel: false,
        warn: true,
        example_code: "
#[proc_macro]
pub fn foo(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    r#\"compile_error!(\"Fish is on fire\")\"#.parse().unwrap()
}
``\u{200B}` ``\u{200B}`
procmacro::foo!();
",
    })
}
