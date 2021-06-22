//! run rust code on the rust-lang playground

use crate::{Error, PrefixContext};

use reqwest::header;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::{borrow::Cow, collections::HashMap};

// ================================
// PLAYGROUND API WRAPPER BEGINS HERE
// ================================

#[derive(Debug, Serialize)]
struct PlaygroundRequest<'a> {
    channel: Channel,
    edition: Edition,
    code: &'a str,
    #[serde(rename = "crateType")]
    crate_type: CrateType,
    mode: Mode,
    tests: bool,
}

#[derive(Debug, Serialize)]
struct MiriRequest<'a> {
    edition: Edition,
    code: &'a str,
}

// has the same fields
type MacroExpansionRequest<'a> = MiriRequest<'a>;

#[derive(Debug, Serialize)]
struct ClippyRequest<'a> {
    edition: Edition,
    #[serde(rename = "crateType")]
    crate_type: CrateType,
    code: &'a str,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Channel {
    Stable,
    Beta,
    Nightly,
}

impl FromStr for Channel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "stable" => Ok(Channel::Stable),
            "beta" => Ok(Channel::Beta),
            "nightly" => Ok(Channel::Nightly),
            _ => Err(format!("invalid release channel `{}`", s).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
enum Edition {
    #[serde(rename = "2015")]
    E2015,
    #[serde(rename = "2018")]
    E2018,
}

impl FromStr for Edition {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "2015" => Ok(Edition::E2015),
            "2018" => Ok(Edition::E2018),
            _ => Err(format!("invalid edition `{}`", s).into()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
enum CrateType {
    #[serde(rename = "bin")]
    Binary,
    #[serde(rename = "lib")]
    Library,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Debug,
    Release,
}

impl FromStr for Mode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "debug" => Ok(Mode::Debug),
            "release" => Ok(Mode::Release),
            _ => Err(format!("invalid compilation mode `{}`", s).into()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct PlayResult {
    success: bool,
    stdout: String,
    stderr: String,
}

/// Returns a gist ID
async fn post_gist(ctx: PrefixContext<'_>, code: &str) -> Result<String, Error> {
    let mut payload = HashMap::new();
    payload.insert("code", code);

    let resp = ctx
        .data
        .http
        .post("https://play.rust-lang.org/meta/gist/")
        .header(header::REFERER, "https://discord.gg/rust-lang-community")
        .json(&payload)
        .send()
        .await?;

    let mut resp: HashMap<String, String> = resp.json().await?;
    log::info!("gist response: {:?}", resp);

    let gist_id = resp.remove("id").ok_or("no gist found")?;
    Ok(gist_id)
}

fn url_from_gist(flags: &CommandFlags, gist_id: &str) -> String {
    format!(
        "https://play.rust-lang.org/?version={}&mode={}&edition={}&gist={}",
        match flags.channel {
            Channel::Nightly => "nightly",
            Channel::Beta => "beta",
            Channel::Stable => "stable",
        },
        match flags.mode {
            Mode::Debug => "debug",
            Mode::Release => "release",
        },
        match flags.edition {
            Edition::E2015 => "2015",
            Edition::E2018 => "2018",
        },
        gist_id
    )
}

// ================================
// UTILITY FUNCTIONS BEGIN HERE
// ================================

// Small thing about multiline strings: while hacking on this file I was unsure how to handle
// trailing newlines in multiline strings:
// - should they have one ("hello\nworld\n")
// - or not? ("hello\nworld")
// After considering several use cases and intensely thinking about it, I arrived at the
// most mathematically sound and natural way: always have a trailing newline, except for the empty
// string. This means, that there'll always be exactly as many newlines as lines, which is
// mathematically sensible. It also means you can also naturally concat multiple multiline
// strings, and `is_empty` will still work.
// So that's how (hopefully) all semantically-multiline strings in this code work

struct CommandFlags {
    channel: Channel,
    mode: Mode,
    edition: Edition,
    warn: bool,
}

/// Returns the parsed flags and a String of parse errors. The parse error string will have a
/// trailing newline (except if empty)
fn parse_flags(args: &poise::KeyValueArgs) -> (CommandFlags, String) {
    let mut errors = String::new();

    let mut flags = CommandFlags {
        channel: Channel::Nightly,
        mode: Mode::Debug,
        edition: Edition::E2018,
        warn: false,
    };

    if let Some(channel) = args.get("channel") {
        match channel.parse() {
            Ok(x) => flags.channel = x,
            Err(e) => errors += &format!("{}\n", e),
        }
    }

    if let Some(mode) = args.get("mode") {
        match mode.parse() {
            Ok(x) => flags.mode = x,
            Err(e) => errors += &format!("{}\n", e),
        }
    }

    if let Some(edition) = args.get("edition") {
        match edition.parse() {
            Ok(x) => flags.edition = x,
            Err(e) => errors += &format!("{}\n", e),
        }
    }

    if let Some(warn) = args.get("warn") {
        match warn.parse() {
            Ok(x) => flags.warn = x,
            Err(e) => errors += &format!("{}\n", e),
        }
    }

    (flags, errors)
}

fn generic_help(
    cmd: &str,
    desc: &str,
    mode_and_channel: bool,
    warn: bool,
    example_code: &str,
) -> String {
    let mut reply = format!(
        "{}. All code is executed on https://play.rust-lang.org.\n",
        desc
    );

    reply += "```rust\n?";
    reply += cmd;
    if mode_and_channel {
        reply += " mode={} channel={}";
    }
    reply += " edition={}";
    if warn {
        reply += " warn={}";
    }
    reply += " ``\u{200B}`";
    reply += example_code;
    reply += "``\u{200B}`\n```\n";

    reply += "Optional arguments:\n";
    if mode_and_channel {
        reply += "- mode: debug, release (default: debug)\n";
        reply += "- channel: stable, beta, nightly (default: nightly)\n";
    }
    reply += "- edition: 2015, 2018 (default: 2018)\n";
    if warn {
        reply += "- warn: true, false (default: false)\n";
    }

    reply
}

/// Strip the input according to a list of start tokens and end tokens. Everything after the start
/// token up to the end token is stripped. Remaining trailing or loading empty lines are removed as
/// well.
///
/// If multiple potential tokens could be used as a stripping point, this function will make the
/// stripped output as compact as possible and choose from the matching tokens accordingly.
// Note to self: don't use "Finished dev" as a parameter to this, because that will break in release
// compilation mode
fn extract_relevant_lines<'a>(
    mut stderr: &'a str,
    strip_start_tokens: &[&str],
    strip_end_tokens: &[&str],
) -> &'a str {
    // Find best matching start token
    if let Some(start_token_pos) = strip_start_tokens
        .iter()
        .filter_map(|t| stderr.rfind(t))
        .max()
    {
        // Keep only lines after that
        stderr = match stderr[start_token_pos..].find('\n') {
            Some(line_end) => &stderr[(line_end + start_token_pos + 1)..],
            None => "",
        };
    }

    // Find best matching end token
    if let Some(end_token_pos) = strip_end_tokens
        .iter()
        .filter_map(|t| stderr.rfind(t))
        .min()
    {
        // Keep only lines before that
        stderr = match stderr[..end_token_pos].rfind('\n') {
            Some(prev_line_end) => &stderr[..=prev_line_end],
            None => "",
        };
    }

    // Strip trailing or leading empty lines
    stderr = stderr.trim_start_matches('\n');
    while stderr.ends_with("\n\n") {
        stderr = &stderr[..(stderr.len() - 1)];
    }

    stderr
}

enum ResultHandling {
    /// Don't consume results at all, making rustc throw an error when the result isn't ()
    None,
    /// Consume using `let _ = { ... };`
    Discard,
    /// Print the result with `println!("{:?}")`
    Print,
}

/// Utility used by the commands to wrap the given code in a `fn main` if not already wrapped.
/// To check, whether a wrap was done, check if the return type is Cow::Borrowed vs Cow::Owned
fn maybe_wrap(code: &str, result_handling: ResultHandling) -> Cow<'_, str> {
    if code.contains("fn main") {
        return Cow::Borrowed(code);
    }

    let mut lines = code.lines().peekable();

    let mut output = String::new();

    // First go through the input lines and extract the crate attributes at the start. Those will
    // be put right at the beginning of the generated code, else they won't work (crate attributes
    // need to be at the top of the file)
    while let Some(line) = lines.peek() {
        let line = line.trim();
        if line.starts_with("#![") {
            output.push_str(line);
            output.push('\n');
        } else if line.is_empty() {
            // do nothing, maybe more crate attributes are coming
        } else {
            break;
        }
        lines.next(); // Advance the iterator
    }

    // fn main boilerplate
    output.push_str(match result_handling {
        ResultHandling::None => "fn main() {\n",
        ResultHandling::Discard => "fn main() { let _ = {\n",
        ResultHandling::Print => "fn main() { println!(\"{:?}\", {\n",
    });

    // Write the rest of the lines that don't contain crate attributes
    for line in lines {
        output.push_str(line);
        output.push('\n');
    }

    // fn main boilerplate counterpart
    output.push_str(match result_handling {
        ResultHandling::None => "}",
        ResultHandling::Discard => "}; }",
        ResultHandling::Print => "}); }",
    });

    Cow::Owned(output)
}

/// Send a Discord reply with the formatted contents of a Playground result
async fn send_reply(
    ctx: PrefixContext<'_>,
    result: PlayResult,
    code: &str,
    flags: &CommandFlags,
    flag_parse_errors: &str,
) -> Result<(), Error> {
    let result = if !result.success {
        result.stderr
    } else if result.stderr.is_empty() {
        result.stdout
    } else {
        format!("{}\n{}", result.stderr, result.stdout)
    };

    if result.trim().is_empty() {
        poise::say_reply(
            poise::Context::Prefix(ctx),
            format!("{}``` ```", flag_parse_errors),
        )
        .await?;
    } else {
        super::reply_potentially_long_text(
            ctx,
            &format!("{}```rust\n{}", flag_parse_errors, result),
            "```",
            &format!(
                "Output too large. Playground link: <{}>",
                url_from_gist(&flags, &post_gist(ctx, code).await?),
            ),
        )
        .await?;
    }

    Ok(())
}

fn apply_rustfmt(text: &str, edition: Edition) -> Result<PlayResult, Error> {
    use std::io::Write as _;

    let mut child = std::process::Command::new("rustfmt")
        .args(&[
            "--edition",
            match edition {
                Edition::E2015 => "2015",
                Edition::E2018 => "2018",
            },
            "--color",
            "never",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    child
        .stdin
        .as_mut()
        .ok_or("This can't happen, we captured by pipe")?
        .write_all(text.as_bytes())?;

    let output = child.wait_with_output()?;
    Ok(PlayResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

// This function must not break when provided non-formatted text with messed up formatting: rustfmt
// may not be installed on the host's computer!
fn strip_fn_main_boilerplate_from_formatted(text: &str) -> String {
    // Remove the fn main boilerplate
    let prefix = "fn main() {";
    let postfix = "}";

    let text =
        if let (Some(prefix_pos), Some(postfix_pos)) = (text.find(prefix), text.rfind(postfix)) {
            text.get((prefix_pos + prefix.len())..postfix_pos)
                .unwrap_or(text)
        } else {
            text
        };
    let text = text.trim();

    // Revert the indent introduced by rustfmt
    let mut output = String::new();
    for line in text.lines() {
        output.push_str(line.strip_prefix("    ").unwrap_or(line));
        output.push('\n');
    }
    output
}

/// Extract compiler output and program stderr output and format the two nicely
fn format_play_eval_stderr(stderr: &str, warn: bool) -> String {
    let compiler_output = extract_relevant_lines(
        &stderr,
        &["Compiling playground"],
        &[
            "warning emitted",
            "warnings emitted",
            "error: aborting",
            "Finished ",
        ],
    );

    if stderr.contains("Running `target") {
        // Program successfully compiled, so compiler output will be just warnings
        let program_stderr = extract_relevant_lines(&stderr, &["Running `target"], &[]);

        if warn {
            // Concatenate compiler output and program stderr with a newline
            match (compiler_output, program_stderr) {
                ("", "") => String::new(),
                (warnings, "") => warnings.to_owned(),
                ("", stderr) => stderr.to_owned(),
                (warnings, stderr) => format!("{}\n{}", warnings, stderr),
            }
        } else {
            program_stderr.to_owned()
        }
    } else {
        // Program didn't get to run, so there must be an error, so we yield the compiler output
        // regardless of whether warn is enabled or not
        compiler_output.to_owned()
    }
}

// ================================
// ACTUAL BOT COMMANDS BEGIN HERE
// ================================

// play and eval work similarly, so this function abstracts over the two
async fn play_or_eval(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
    result_handling: ResultHandling,
) -> Result<(), Error> {
    let code = maybe_wrap(&code.code, result_handling);
    let (flags, flag_parse_errors) = parse_flags(&flags);

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
    play_or_eval(ctx, flags, code, ResultHandling::None).await
}

pub fn play_help() -> String {
    generic_help("play", "Compile and run Rust code", true, true, "code")
}

/// Evaluate a single Rust expression
#[poise::command(track_edits, broadcast_typing, explanation_fn = "eval_help")]
pub async fn eval(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    play_or_eval(ctx, flags, code, ResultHandling::Print).await
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
