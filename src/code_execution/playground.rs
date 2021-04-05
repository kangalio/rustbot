//! run rust code on the rust-lang playground

use crate::{Context, Error};

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
async fn post_gist(ctx: Context<'_>, code: &str) -> Result<String, Error> {
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

fn generic_help(cmd: &str, desc: &str, full: bool, example_code: &str) -> String {
    let mut reply = format!(
        "{}. All code is executed on https://play.rust-lang.org.\n",
        desc
    );

    reply += &format!(
        "```?{} {}edition={{}} ``\u{200B}`{}``\u{200B}` ```\n",
        cmd,
        if full { "mode={} channel={} " } else { "" },
        example_code,
    );

    reply += "Optional arguments:\n";
    if full {
        reply += "    \tmode: debug, release (default: debug)\n";
        reply += "    \tchannel: stable, beta, nightly (default: nightly)\n";
    }
    reply += "    \tedition: 2015, 2018 (default: 2018)\n";

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
    ctx: Context<'_>,
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
        poise::say_reply(ctx, format!("{}``` ```", flag_parse_errors)).await?;
    } else {
        super::reply_potentially_long_text(
            ctx,
            &format!("{}```rust\n{}", flag_parse_errors, result),
            "```",
            &format!(
                "Output too large. Playground link: {}",
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

fn strip_fn_main_boilerplate_from_formatted(text: &str) -> String {
    // Remove all the fn main boilerplate and also revert the indent introduced by rustfmt
    let mut output = String::new();
    for line in extract_relevant_lines(text, &["fn main() {"], &["}"]).lines() {
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
    ctx: Context<'_>,
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
    ctx: Context<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    play_or_eval(ctx, flags, code, ResultHandling::None).await
}

pub fn play_help() -> String {
    generic_help("play", "Compile and run Rust code", true, "code")
}

/// Evaluate a single Rust expression
#[poise::command(track_edits, broadcast_typing, explanation_fn = "eval_help")]
pub async fn eval(
    ctx: Context<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    play_or_eval(ctx, flags, code, ResultHandling::Print).await
}

pub fn eval_help() -> String {
    generic_help("eval", "Compile and run Rust code", true, "code")
}

/// Run code and detect undefined behavior using Miri
#[poise::command(track_edits, broadcast_typing, explanation_fn = "miri_help")]
pub async fn miri(
    ctx: Context<'_>,
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
    generic_help("miri", desc, false, "code")
}

/// Expand macros to their raw desugared form
#[poise::command(broadcast_typing, track_edits, explanation_fn = "expand_help")]
pub async fn expand(
    ctx: Context<'_>,
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
    generic_help("expand", desc, false, "code")
}

/// Catch common mistakes using the Clippy linter
#[poise::command(broadcast_typing, track_edits, explanation_fn = "clippy_help")]
pub async fn clippy(
    ctx: Context<'_>,
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
    generic_help("clippy", desc, false, "code")
}

/// Format code using rustfmt
#[poise::command(broadcast_typing, track_edits, explanation_fn = "fmt_help")]
pub async fn fmt(
    ctx: Context<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let code = &maybe_wrap(&code.code, ResultHandling::None);
    let was_fn_main_wrapped = matches!(code, Cow::Owned(_));
    let (flags, flag_parse_errors) = parse_flags(&flags);

    let mut result = apply_rustfmt(&code, flags.edition)?;
    if was_fn_main_wrapped {
        result.stdout = strip_fn_main_boilerplate_from_formatted(&result.stdout);
    }

    send_reply(ctx, result, code, &flags, &flag_parse_errors).await
}

pub fn fmt_help() -> String {
    let desc = "Format code using rustfmt";
    generic_help("fmt", desc, false, "code")
}

/// Benchmark small snippets of code
#[poise::command(broadcast_typing, track_edits, explanation_fn = "microbench_help")]
pub async fn microbench(
    ctx: Context<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let user_code = &code.code;

    let mut code =
        // include convenience import for users
        "#![feature(test)] #[allow(unused_imports)] use std::hint::black_box;\n".to_owned();

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
            ctx,
            "No public functions found for benchmarking :thinking:".into(),
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
        "Benchmark small snippets of code by running them repeatedly. The public function snippets are run \
        in chunks, interleaved: Snippet A is ran 10000 times, then snippet B is ran 10000 times, \
        then snippet A again, and so on until a certain time has passed. After that, the \
        measuremants are averaged and the standard deviation is calculated for each";
    generic_help(
        "microbench",
        desc,
        false,
        "
pub fn snippet_a() { /* code */ }
pub fn snippet_b() { /* code */ }
",
    )
}
