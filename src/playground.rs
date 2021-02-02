//! run rust code on the rust-lang playground

use crate::{api, commands::Args, Error};

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
fn post_gist(args: &Args, code: &str) -> Result<String, Error> {
    let mut payload = HashMap::new();
    payload.insert("code", code);

    let resp = args
        .http
        .post("https://play.rust-lang.org/meta/gist/")
        .header(header::REFERER, "https://discord.gg/rust-lang")
        .json(&payload)
        .send()?;

    let mut resp: HashMap<String, String> = resp.json()?;
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

struct CommandFlags {
    channel: Channel,
    mode: Mode,
    edition: Edition,
}

/// Returns the parsed flags and a String of parse errors
fn parse_flags(args: &Args) -> (CommandFlags, String) {
    let mut errors = String::new();

    let mut flags = CommandFlags {
        channel: Channel::Nightly,
        mode: Mode::Debug,
        edition: Edition::E2018,
    };

    if let Some(channel) = args.params.get("channel") {
        match channel.parse() {
            Ok(c) => flags.channel = c,
            Err(e) => errors += &format!("{}\n", e),
        }
    }

    if let Some(mode) = args.params.get("mode") {
        match mode.parse() {
            Ok(m) => flags.mode = m,
            Err(e) => errors += &format!("{}\n", e),
        }
    }

    if let Some(edition) = args.params.get("edition") {
        match edition.parse() {
            Ok(e) => flags.edition = e,
            Err(e) => errors += &format!("{}\n", e),
        }
    }

    (flags, errors)
}

fn generic_help(args: &Args, cmd: &str, desc: &str, full: bool) -> Result<(), Error> {
    let mut reply = format!(
        "{}. All code is executed on https://play.rust-lang.org.\n",
        desc
    );

    reply += &format!(
        "```?{} {}edition={{}} ``\u{200B}`code``\u{200B}` ```\n",
        cmd,
        if full { "mode={} channel={} " } else { "" },
    );

    reply += "Optional arguments:\n";
    if full {
        reply += "    \tmode: debug, release (default: debug)\n";
        reply += "    \tchannel: stable, beta, nightly (default: nightly)\n";
    }
    reply += "    \tedition: 2015, 2018 (default: 2018)\n";

    api::send_reply(args, &reply)
}

/// Strip the input so that only the lines from the first matching strip_start_token up to the
/// first matching strip_end_token remain. The lines with the tokens themselves are stripped as
/// well.
fn extract_relevant_lines<'a>(
    mut stderr: &'a str,
    strip_start_tokens: &[&str],
    strip_end_tokens: &[&str],
) -> &'a str {
    for token in strip_start_tokens {
        if let Some(token_start) = stderr.find(token) {
            stderr = match stderr[token_start..].find('\n') {
                Some(line_end) => &stderr[(line_end + token_start + 1)..],
                None => "",
            };
        }
    }

    for token in strip_end_tokens {
        if let Some(token_start) = stderr.rfind(token) {
            stderr = match stderr[..token_start].rfind('\n') {
                Some(prev_line_end) => &stderr[..prev_line_end],
                None => "",
            };
        }
    }

    stderr
}

enum ResultHandling {
    // /// Don't consume results at all, which makes rustc throw an error when the result isn't ()
    // None,
    /// Consume using `let _ = { ... };`
    Discard,
    /// Print the result with `println!("{:?}")`
    Print,
}

/// Utility used by the commands to wrap the given code in a `fn main`, if it isn't already
fn maybe_wrap(code: &str, result_handling: ResultHandling) -> Cow<'_, str> {
    if code.contains("fn main") {
        Cow::Borrowed(code)
    } else {
        let (start, end) = match result_handling {
            ResultHandling::Discard => ("fn main() { let _ = {\n", "}; }"),
            ResultHandling::Print => ("fn main() { println!(\"{:?}\", {\n", "}); }"),
        };

        let mut output = String::from(start);
        for line in code.lines() {
            output.push_str("        ");
            output.push_str(line);
            output.push_str("\n");
        }
        output.push_str(end);

        Cow::Owned(output)
    }
}

/// Send a Discord reply with the formatted contents of a Playground result
fn send_reply(
    args: &Args<'_>,
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
        api::send_reply(&args, &format!("{}``` ```", flag_parse_errors))
    } else {
        crate::reply_potentially_long_text(
            &args,
            &format!("{}```rust\n{}", flag_parse_errors, result),
            "```",
            &format!(
                "Output too large. Playground link: {}",
                url_from_gist(&flags, &post_gist(&args, code)?),
            ),
        )
    }
}

// ================================
// ACTUAL BOT COMMANDS BEGIN HERE
// ================================

// play and eval work similarly, so this function abstracts over the two
pub fn play_or_eval(args: &Args, attempt_to_wrap_and_display: bool) -> Result<(), Error> {
    let code = match attempt_to_wrap_and_display {
        true => maybe_wrap(crate::extract_code(args.body)?, ResultHandling::Print),
        false => Cow::Borrowed(crate::extract_code(args.body)?),
    };
    let (flags, flag_parse_errors) = parse_flags(args);

    let mut result: PlayResult = args
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
        .send()?
        .json()?;

    result.stderr =
        extract_relevant_lines(&result.stderr, &["Running `target"], &["error: aborting"])
            .to_owned();

    send_reply(args, result, &code, &flags, &flag_parse_errors)
}

pub fn play(args: &Args) -> Result<(), Error> {
    play_or_eval(args, false)
}

pub fn eval(args: &Args) -> Result<(), Error> {
    play_or_eval(args, true)
}

pub fn play_and_eval_help(args: &Args, name: &str) -> Result<(), Error> {
    generic_help(&args, name, "Compile and run Rust code", true)
}

pub fn miri(args: &Args) -> Result<(), Error> {
    let code = &maybe_wrap(crate::extract_code(args.body)?, ResultHandling::Discard);
    let (flags, flag_parse_errors) = parse_flags(&args);

    let mut result: PlayResult = args
        .http
        .post("https://play.rust-lang.org/miri")
        .json(&MiriRequest {
            code,
            edition: flags.edition,
        })
        .send()?
        .json()?;

    result.stderr = extract_relevant_lines(
        &result.stderr,
        &["Running `/playground"],
        &["error: aborting"],
    )
    .to_owned();

    send_reply(args, result, code, &flags, &flag_parse_errors)
}

pub fn miri_help(args: &Args) -> Result<(), Error> {
    let desc = "Execute this program in the Miri interpreter to detect certain cases of undefined behavior (like out-of-bounds memory access)";
    generic_help(&args, "miri", desc, false)
}

fn apply_rustfmt(text: &str) -> Result<String, Error> {
    use std::io::Write as _;

    let mut child = std::process::Command::new("rustfmt")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    child
        .stdin
        .as_mut()
        .ok_or("This can't happen, we captured by pipe")?
        .write_all(text.as_bytes())?;

    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into())
    }
}

pub fn expand_macros(args: &Args) -> Result<(), Error> {
    let code = crate::extract_code(args.body)?;
    let (flags, flag_parse_errors) = parse_flags(&args);

    let (code, was_fn_main_wrapped) = if code.contains("fn main") {
        (Cow::Borrowed(code), false)
    } else {
        let mut output = String::from("fn main() {\n");
        for line in code.lines() {
            output.push_str("    ");
            output.push_str(line);
            output.push_str("\n");
        }
        output.push_str("}");
        (Cow::Owned(output), true)
    };

    let mut result: PlayResult = args
        .http
        .post("https://play.rust-lang.org/macro-expansion")
        .json(&MacroExpansionRequest {
            code: &code,
            edition: flags.edition,
        })
        .send()?
        .json()?;

    result.stderr = extract_relevant_lines(
        &result.stderr,
        &["Finished dev", "Compiling playground"],
        &["error: aborting"],
    )
    .to_owned();

    result.stdout = apply_rustfmt(&result.stdout).map_err(|e| {
        warn!("Couldn't run rustfmt: {}", e);
        e
    })?;

    result.stdout = if was_fn_main_wrapped {
        // Remove all the fn main boilerplate and also dedent appropriately
        let mut output = String::new();
        for line in extract_relevant_lines(&result.stdout, &["fn main() {"], &["}"]).lines() {
            output.push_str(line.strip_prefix("    ").unwrap_or(line));
            output.push_str("\n");
        }
        output
    } else {
        result.stdout
    };

    send_reply(args, result, &code, &flags, &flag_parse_errors)
}

pub fn expand_macros_help(args: &Args) -> Result<(), Error> {
    let desc = "Expand macros to their raw desugared form";
    generic_help(&args, "expand", desc, false)
}

pub fn clippy(args: &Args) -> Result<(), Error> {
    let code = &maybe_wrap(crate::extract_code(args.body)?, ResultHandling::Discard);
    let (flags, flag_parse_errors) = parse_flags(&args);

    let mut result: PlayResult = args
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
        .send()?
        .json()?;

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

    send_reply(args, result, code, &flags, &flag_parse_errors)
}

pub fn clippy_help(args: &Args) -> Result<(), Error> {
    let desc = "Catch common mistakes and improve the code using the Clippy linter";
    generic_help(&args, "clippy", desc, false)
}
