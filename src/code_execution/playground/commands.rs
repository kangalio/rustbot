use super::{api::*, util::*};
use crate::{Error, PrefixContext};

use std::borrow::Cow;

// play and eval work similarly, so this function abstracts over the two
async fn play_or_eval(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    force_warnings: bool, // If true, force enable warnings regardless of flags
    code: poise::CodeBlock,
    result_handling: ResultHandling,
) -> Result<(), Error> {
    let code = maybe_wrap(&code.code, result_handling);
    let (mut flags, flag_parse_errors) = parse_flags(&flags);

    if force_warnings {
        flags.warn = true;
    }

    let mut result: PlayResult = ctx
        .data
        .http
        .post("https://play.rust-lang.org/execute")
        .json(&PlaygroundRequest {
            code: &code,
            channel: flags.channel,
            crate_type: if code.contains("fn main") {
                CrateType::Binary
            } else {
                CrateType::Library
            },
            edition: flags.edition,
            mode: flags.mode,
            tests: false,
        })
        .send()
        .await?
        .json()
        .await?;

    result.stderr = format_play_eval_stderr(&result.stderr, flags.warn);

    send_reply(ctx, result, &code, &flags, &flag_parse_errors).await
}

/// Compile and run Rust code in a playground
#[poise::command(track_edits, broadcast_typing, explanation_fn = "play_help")]
pub async fn play(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    play_or_eval(ctx, flags, false, code, ResultHandling::None).await
}

pub fn play_help() -> String {
    generic_help("play", "Compile and run Rust code", true, true, "code")
}

/// Compile and run Rust code with warnings
#[poise::command(
    track_edits,
    broadcast_typing,
    hide_in_help, // don't clutter help menu with something that ?play can do too
    explanation_fn = "playwarn_help"
)]
pub async fn playwarn(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    play_or_eval(ctx, flags, true, code, ResultHandling::None).await
}

pub fn playwarn_help() -> String {
    generic_help(
        "playwarn",
        "Compile and run Rust code with warnings. Equivalent to `?play warn=true`",
        true,
        false,
        "code",
    )
}

/// Evaluate a single Rust expression
#[poise::command(track_edits, broadcast_typing, explanation_fn = "eval_help")]
pub async fn eval(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    play_or_eval(ctx, flags, false, code, ResultHandling::Print).await
}

pub fn eval_help() -> String {
    generic_help("eval", "Compile and run Rust code", true, true, "code")
}

/// Run code and detect undefined behavior using Miri
#[poise::command(track_edits, broadcast_typing, explanation_fn = "miri_help")]
pub async fn miri(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let code = &maybe_wrap(&code.code, ResultHandling::Discard);
    let (flags, flag_parse_errors) = parse_flags(&flags);

    let mut result: PlayResult = ctx
        .data
        .http
        .post("https://play.rust-lang.org/miri")
        .json(&MiriRequest {
            code,
            edition: flags.edition,
        })
        .send()
        .await?
        .json()
        .await?;

    result.stderr = extract_relevant_lines(
        &result.stderr,
        &["Running `/playground"],
        &["error: aborting"],
    )
    .to_owned();

    send_reply(ctx, result, code, &flags, &flag_parse_errors).await
}

pub fn miri_help() -> String {
    let desc = "Execute this program in the Miri interpreter to detect certain cases of undefined behavior (like out-of-bounds memory access)";
    // Playgrounds sends miri warnings/errors and output in the same field so we can't filter
    // warnings out
    generic_help("miri", desc, false, false, "code")
}

/// Expand macros to their raw desugared form
#[poise::command(broadcast_typing, track_edits, explanation_fn = "expand_help")]
pub async fn expand(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let code = maybe_wrap(&code.code, ResultHandling::None);
    let was_fn_main_wrapped = matches!(code, Cow::Owned(_));
    let (flags, flag_parse_errors) = parse_flags(&flags);

    let mut result: PlayResult = ctx
        .data
        .http
        .post("https://play.rust-lang.org/macro-expansion")
        .json(&MacroExpansionRequest {
            code: &code,
            edition: flags.edition,
        })
        .send()
        .await?
        .json()
        .await?;

    result.stderr = extract_relevant_lines(
        &result.stderr,
        &["Finished ", "Compiling playground"],
        &["error: aborting"],
    )
    .to_owned();

    if result.success {
        match apply_rustfmt(&result.stdout, flags.edition) {
            Ok(PlayResult { success: true, stdout, .. }) => result.stdout = stdout,
            Ok(PlayResult { success: false, stderr, .. }) => log::warn!("Huh, rustfmt failed even though this code successfully passed through macro expansion before: {}", stderr),
            Err(e) => log::warn!("Couldn't run rustfmt: {}", e),
        }
    }
    if was_fn_main_wrapped {
        result.stdout = strip_fn_main_boilerplate_from_formatted(&result.stdout);
    }

    send_reply(ctx, result, &code, &flags, &flag_parse_errors).await
}

pub fn expand_help() -> String {
    let desc = "Expand macros to their raw desugared form";
    generic_help("expand", desc, false, false, "code")
}

/// Catch common mistakes using the Clippy linter
#[poise::command(broadcast_typing, track_edits, explanation_fn = "clippy_help")]
pub async fn clippy(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let code = &maybe_wrap(&code.code, ResultHandling::Discard);
    let (flags, flag_parse_errors) = parse_flags(&flags);

    let mut result: PlayResult = ctx
        .data
        .http
        .post("https://play.rust-lang.org/clippy")
        .json(&ClippyRequest {
            code,
            edition: flags.edition,
            crate_type: if code.contains("fn main") {
                CrateType::Binary
            } else {
                CrateType::Library
            },
        })
        .send()
        .await?
        .json()
        .await?;

    result.stderr = extract_relevant_lines(
        &result.stderr,
        &["Checking playground", "Running `/playground"],
        &[
            "error: aborting",
            "1 warning emitted",
            "warnings emitted",
            "Finished ",
        ],
    )
    .to_owned();

    send_reply(ctx, result, code, &flags, &flag_parse_errors).await
}

pub fn clippy_help() -> String {
    let desc = "Catch common mistakes and improve the code using the Clippy linter";
    generic_help("clippy", desc, false, false, "code")
}

/// Format code using rustfmt
#[poise::command(broadcast_typing, track_edits, explanation_fn = "fmt_help")]
pub async fn fmt(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let code = &maybe_wrap(&code.code, ResultHandling::None);
    let was_fn_main_wrapped = matches!(code, Cow::Owned(_));
    let (flags, flag_parse_errors) = parse_flags(&flags);

    let mut result = apply_rustfmt(&code, flags.edition)
        .map_err(|e| format!("Error while executing rustfmt: {}", e))?;
    if was_fn_main_wrapped {
        result.stdout = strip_fn_main_boilerplate_from_formatted(&result.stdout);
    }

    send_reply(ctx, result, code, &flags, &flag_parse_errors).await
}

pub fn fmt_help() -> String {
    let desc = "Format code using rustfmt";
    generic_help("fmt", desc, false, false, "code")
}

/// Benchmark small snippets of code
#[poise::command(broadcast_typing, track_edits, explanation_fn = "microbench_help")]
pub async fn microbench(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let user_code = &code.code;

    let mut code =
        // include convenience import for users
        "#![feature(bench_black_box)] #[allow(unused_imports)] use std::hint::black_box;\n".to_owned();

    let black_box_hint = !user_code.contains("black_box");
    code += user_code;

    code += r#"
fn bench(functions: &[(&str, fn())]) {
    const CHUNK_SIZE: usize = 10000;

    // Warm up
    for (_, function) in functions.iter() {
        for _ in 0..CHUNK_SIZE {
            (function)();
        }
    }

    let mut functions_chunk_times = functions.iter().map(|_| Vec::new()).collect::<Vec<_>>();

    let start = std::time::Instant::now();
    while (std::time::Instant::now() - start).as_secs() < 5 {
        for (chunk_times, (_, function)) in functions_chunk_times.iter_mut().zip(functions) {
            let start = std::time::Instant::now();
            for _ in 0..CHUNK_SIZE {
                (function)();
            }
            chunk_times.push((std::time::Instant::now() - start).as_secs_f64() / CHUNK_SIZE as f64);
        }
    }

    for (chunk_times, (function_name, _)) in functions_chunk_times.iter().zip(functions) {
        let mean_time: f64 = chunk_times.iter().sum::<f64>() / chunk_times.len() as f64;
        let standard_deviation: f64 = f64::sqrt(
            chunk_times
                .iter()
                .map(|time| (time - mean_time).powi(2))
                .sum::<f64>()
                / chunk_times.len() as f64,
        );

        println!(
            "{}: {:.0} iters per second ({:.1}ns±{:.1})",
            function_name,
            1.0 / mean_time,
            mean_time * 1_000_000_000.0,
            standard_deviation * 1_000_000_000.0,
        );
    }
}

fn main() {
"#;

    let pub_fn_indices = user_code.match_indices("pub fn ");
    if pub_fn_indices.clone().count() == 0 {
        poise::say_reply(
            poise::Context::Prefix(ctx),
            "No public functions (`pub fn`) found for benchmarking :thinking:".into(),
        )
        .await?;
        return Ok(());
    }

    code += "bench(&[";
    for (index, _) in pub_fn_indices {
        let function_name_start = index + "pub fn ".len();
        let function_name_end = match user_code[function_name_start..].find('(') {
            Some(x) => x + function_name_start,
            None => continue,
        };
        let function_name = user_code[function_name_start..function_name_end].trim();

        code += &format!("(\"{0}\", {0}), ", function_name);
    }
    code += "]);\n}\n";

    let (flags, mut flag_parse_errors) = parse_flags(&flags);
    let mut result: PlayResult = ctx
        .data
        .http
        .post("https://play.rust-lang.org/execute")
        .json(&PlaygroundRequest {
            code: &code,
            channel: Channel::Nightly, // has to be, for black_box
            crate_type: if code.contains("fn main") {
                CrateType::Binary
            } else {
                CrateType::Library
            },
            edition: flags.edition,
            mode: Mode::Release, // benchmarks on debug don't make sense
            tests: false,
        })
        .send()
        .await?
        .json()
        .await?;

    result.stderr = format_play_eval_stderr(&result.stderr, flags.warn);

    if black_box_hint {
        flag_parse_errors +=
            "Hint: use the black_box function to prevent computations from being optimized out\n";
    }
    send_reply(ctx, result, &code, &flags, &flag_parse_errors).await
}

pub fn microbench_help() -> String {
    let desc =
        "Benchmark small snippets of code by running them repeatedly. Public function snippets are \
        run in blocks of 10000 repetitions in a cycle until a certain time has passed. Measurements \
        are averaged and standard deviation is calculated for each";
    generic_help(
        "microbench",
        desc,
        false,
        true,
        "
pub fn snippet_a() { /* code */ }
pub fn snippet_b() { /* code */ }
",
    )
}

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
    let desc = "Compile and use a procedural macro by providing two snippets: one for the \
        proc-macro code, and one for the usage code which can refer to the proc-macro crate as \
        `procmacro`";
    generic_help(
        "procmacro",
        desc,
        false,
        true,
        "
#[proc_macro]
pub fn foo(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    r#\"compile_error!(\"Fish is on fire\")\"#.parse().unwrap()
}
``\u{200B}` ``\u{200B}`
procmacro::foo!();
",
    )
}
