//! run rust code on the rust-lang playground

use std::str::FromStr;
use std::{borrow::Cow, collections::HashMap};

use reqwest::header;
use serde::{Deserialize, Serialize};
use serenity::model::prelude::*;
use serenity_framework::prelude::*;

use crate::{api, Context, Error};

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

#[derive(Debug)]
struct SimpleError(String);
impl std::fmt::Display for SimpleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SimpleError {}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Channel {
    Stable,
    Beta,
    Nightly,
}

impl FromStr for Channel {
    type Err = SimpleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stable" => Ok(Channel::Stable),
            "beta" => Ok(Channel::Beta),
            "nightly" => Ok(Channel::Nightly),
            _ => Err(SimpleError(format!("invalid release channel `{}`", s))),
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
    type Err = SimpleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "2015" => Ok(Edition::E2015),
            "2018" => Ok(Edition::E2018),
            _ => Err(SimpleError(format!("invalid edition `{}`", s))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum CrateType {
    Bin,
    Lib,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Debug,
    Release,
}

impl FromStr for Mode {
    type Err = SimpleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "debug" => Ok(Mode::Debug),
            "release" => Ok(Mode::Release),
            _ => Err(SimpleError(format!("invalid compilation mode `{}`", s))),
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
async fn post_gist(ctx: &Context, code: &str) -> Result<String, Error> {
    let mut payload = HashMap::new();
    payload.insert("code", code);

    let resp = ctx
        .data
        .reqwest
        .post("https://play.rust-lang.org/meta/gist/")
        .header(header::REFERER, "https://discord.gg/rust-lang")
        .json(&payload)
        .send()
        .await?;

    let mut resp: HashMap<String, String> = resp.json().await?;
    info!("gist response: {:?}", resp);

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
}

/// Returns the parsed flags and a String of parse errors
fn parse_flags(
    channel: Option<Channel>,
    mode: Option<Mode>,
    edition: Option<Edition>,
) -> CommandFlags {
    CommandFlags {
        channel: channel.unwrap_or(Channel::Nightly),
        mode: mode.unwrap_or(Mode::Debug),
        edition: edition.unwrap_or(Edition::E2018),
    }
}

/// Strip the input according to a list of start tokens and end tokens. Everything after the start
/// token up to the end token is stripped. Remaining trailing or loading empty lines are removed as
/// well.
///
/// If multiple potential tokens could be used as a stripping point, this function will make the
/// stripped output as compact as possible and choose from the matching tokens accordingly.
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
    ctx: &Context,
    msg: &Message,
    result: PlayResult,
    code: &str,
    flags: &CommandFlags,
) -> Result<(), Error> {
    let result = if !result.success {
        result.stderr
    } else if result.stderr.is_empty() {
        result.stdout
    } else {
        format!("{}\n{}", result.stderr, result.stdout)
    };

    if result.trim().is_empty() {
        api::send_reply(ctx, msg, "``` ```").await
    } else {
        crate::reply_potentially_long_text(
            &ctx,
            msg,
            &format!("```rust\n{}", result),
            "```",
            &format!(
                "Output too large. Playground link: {}",
                url_from_gist(&flags, &post_gist(&ctx, code).await?),
            ),
        )
        .await
    }
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

// ================================
// ACTUAL BOT COMMANDS BEGIN HERE
// ================================

// play and eval work similarly, so this function abstracts over the two
async fn play_or_eval(
    ctx: &Context,
    msg: &Message,
    result_handling: ResultHandling,
    flags: CommandFlags,
    code_block: &str,
) -> Result<(), Error> {
    let code = maybe_wrap(crate::extract_code(code_block)?, result_handling);

    let mut result: PlayResult = ctx
        .data
        .reqwest
        .post("https://play.rust-lang.org/execute")
        .json(&PlaygroundRequest {
            code: &code,
            channel: flags.channel,
            crate_type: if code.contains("fn main") {
                CrateType::Bin
            } else {
                CrateType::Lib
            },
            edition: flags.edition,
            mode: flags.mode,
            tests: false,
        })
        .send()
        .await?
        .json()
        .await?;

    let compiler_warnings = extract_relevant_lines(
        &result.stderr,
        &["Compiling playground"],
        &[
            "warning emitted",
            "warnings emitted",
            "error: aborting",
            "Finished dev",
        ],
    );
    let program_stderr = match result.stderr.contains("Running `target") {
        true => extract_relevant_lines(&result.stderr, &["Running `target"], &[]),
        false => "",
    };

    result.stderr = match (compiler_warnings, program_stderr) {
        ("", "") => String::new(),
        (warnings, "") => warnings.to_owned(),
        ("", stderr) => stderr.to_owned(),
        (warnings, stderr) => format!("{}\n{}", warnings, stderr),
    };

    send_reply(&ctx, msg, result, &code, &flags).await
}

#[command]
/// Compile and run Rust code
pub async fn play(
    ctx: Context,
    msg: &Message,
    channel: Option<Channel>,
    mode: Option<Mode>,
    edition: Option<Edition>,
    #[rest] code_block: String,
) -> Result<(), Error> {
    play_or_eval(
        &ctx,
        msg,
        ResultHandling::None,
        parse_flags(channel, mode, edition),
        &code_block,
    )
    .await
}

#[command]
/// Compile and run Rust code and print the result
pub async fn eval(
    ctx: Context,
    msg: &Message,
    channel: Option<Channel>,
    mode: Option<Mode>,
    edition: Option<Edition>,
    #[rest] code_block: String,
) -> Result<(), Error> {
    play_or_eval(
        &ctx,
        msg,
        ResultHandling::Print,
        parse_flags(channel, mode, edition),
        &code_block,
    )
    .await
}

#[command]
/// Run code and detect undefined behavior using Miri
/// Miri can detect certain cases of undefined behavior like out-of-bounds memory access. It can be
/// quite helpful to verify the correctness of unsafe code.
pub async fn miri(
    ctx: Context,
    msg: &Message,
    edition: Option<Edition>,
    #[rest] code_block: String,
) -> Result<(), Error> {
    let code = &maybe_wrap(crate::extract_code(&code_block)?, ResultHandling::Discard);
    let flags = parse_flags(None, None, edition);

    let mut result: PlayResult = ctx
        .data
        .reqwest
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

    send_reply(&ctx, msg, result, code, &flags).await
}

#[command]
/// Expand macros to their raw desugared form
pub async fn expand_macros(
    ctx: Context,
    msg: &Message,
    edition: Option<Edition>,
    #[rest] code_block: String,
) -> Result<(), Error> {
    let code = maybe_wrap(crate::extract_code(&code_block)?, ResultHandling::None);
    let was_fn_main_wrapped = matches!(code, Cow::Owned(_));
    let flags = parse_flags(None, None, edition);

    let mut result: PlayResult = ctx
        .data
        .reqwest
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
        &["Finished dev", "Compiling playground"],
        &["error: aborting"],
    )
    .to_owned();

    if result.success {
        match apply_rustfmt(&result.stdout, flags.edition) {
            Ok(PlayResult { success: true, stdout, .. }) => result.stdout = stdout,
            Ok(PlayResult { success: false, stderr, .. }) => warn!("Huh, rustfmt failed even though this code successfully passed through macro expansion before: {}", stderr),
            Err(e) => warn!("Couldn't run rustfmt: {}", e),
        }
    }
    if was_fn_main_wrapped {
        result.stdout = strip_fn_main_boilerplate_from_formatted(&result.stdout);
    }

    send_reply(&ctx, msg, result, &code, &flags).await
}

#[command]
/// Catch common mistakes using the Clippy linter
pub async fn clippy(
    ctx: Context,
    msg: &Message,
    edition: Option<Edition>,
    #[rest] code_block: String,
) -> Result<(), Error> {
    let code = &maybe_wrap(crate::extract_code(&code_block)?, ResultHandling::Discard);
    let flags = parse_flags(None, None, edition);

    let mut result: PlayResult = ctx
        .data
        .reqwest
        .post("https://play.rust-lang.org/clippy")
        .json(&ClippyRequest {
            code,
            edition: flags.edition,
            crate_type: if code.contains("fn main") {
                CrateType::Bin
            } else {
                CrateType::Lib
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
            "Finished dev",
        ],
    )
    .to_owned();

    send_reply(&ctx, msg, result, code, &flags).await
}

#[command]
/// Format code using rustfmt
pub async fn fmt(
    ctx: Context,
    msg: &Message,
    edition: Option<Edition>,
    #[rest] code_block: String,
) -> Result<(), Error> {
    let code = &maybe_wrap(crate::extract_code(&code_block)?, ResultHandling::None);
    let was_fn_main_wrapped = matches!(code, Cow::Owned(_));
    let flags = parse_flags(None, None, edition);

    let mut result = apply_rustfmt(&code, flags.edition)?;
    if was_fn_main_wrapped {
        result.stdout = strip_fn_main_boilerplate_from_formatted(&result.stdout);
    }

    send_reply(&ctx, msg, result, code, &flags).await
}
