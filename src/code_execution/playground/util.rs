use super::api;
use crate::{Error, PrefixContext};

use std::borrow::Cow;

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

/// Returns the parsed flags and a String of parse errors. The parse error string will have a
/// trailing newline (except if empty)
pub fn parse_flags(args: &poise::KeyValueArgs) -> (api::CommandFlags, String) {
    let mut errors = String::new();

    let mut flags = api::CommandFlags {
        channel: api::Channel::Nightly,
        mode: api::Mode::Debug,
        edition: api::Edition::E2018,
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

pub struct GenericHelp<'a> {
    pub command: &'a str,
    pub desc: &'a str,
    pub mode_and_channel: bool,
    pub warn: bool,
    pub example_code: &'a str,
}

pub fn generic_help(spec: GenericHelp<'_>) -> String {
    let mut reply = format!(
        "{}. All code is executed on https://play.rust-lang.org.\n",
        spec.desc
    );

    reply += "```rust\n?";
    reply += spec.command;
    if spec.mode_and_channel {
        reply += " mode={} channel={}";
    }
    reply += " edition={}";
    if spec.warn {
        reply += " warn={}";
    }
    reply += " ``\u{200B}`";
    reply += spec.example_code;
    reply += "``\u{200B}`\n```\n";

    reply += "Optional arguments:\n";
    if spec.mode_and_channel {
        reply += "- mode: debug, release (default: debug)\n";
        reply += "- channel: stable, beta, nightly (default: nightly)\n";
    }
    reply += "- edition: 2015, 2018 (default: 2018)\n";
    if spec.warn {
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
pub fn extract_relevant_lines<'a>(
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

pub enum ResultHandling {
    /// Don't consume results at all, making rustc throw an error when the result isn't ()
    None,
    /// Consume using `let _ = { ... };`
    Discard,
    /// Print the result with `println!("{:?}")`
    Print,
}

pub fn hoise_crate_attributes(code: &str, after_crate_attrs: &str, after_code: &str) -> String {
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

    output.push_str(after_crate_attrs);

    // Write the rest of the lines that don't contain crate attributes
    for line in lines {
        output.push_str(line);
        output.push('\n');
    }

    output.push_str(after_code);

    output
}

/// Utility used by the commands to wrap the given code in a `fn main` if not already wrapped.
/// To check, whether a wrap was done, check if the return type is Cow::Borrowed vs Cow::Owned
/// If a wrap was done, also hoists crate attributes to the top so they keep working
pub fn maybe_wrap(code: &str, result_handling: ResultHandling) -> Cow<'_, str> {
    if code.contains("fn main") {
        return Cow::Borrowed(code);
    }

    // fn main boilerplate
    let after_crate_attrs = match result_handling {
        ResultHandling::None => "fn main() {\n",
        ResultHandling::Discard => "fn main() { let _ = {\n",
        ResultHandling::Print => "fn main() { println!(\"{:?}\", {\n",
    };

    // fn main boilerplate counterpart
    let after_code = match result_handling {
        ResultHandling::None => "}",
        ResultHandling::Discard => "}; }",
        ResultHandling::Print => "}); }",
    };

    Cow::Owned(hoise_crate_attributes(code, after_crate_attrs, after_code))
}

/// Send a Discord reply with the formatted contents of a Playground result
pub async fn send_reply(
    ctx: PrefixContext<'_>,
    result: api::PlayResult,
    code: &str,
    flags: &api::CommandFlags,
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
                api::url_from_gist(&flags, &api::post_gist(ctx, code).await?),
            ),
        )
        .await?;
    }

    Ok(())
}

pub fn apply_rustfmt(text: &str, edition: api::Edition) -> Result<api::PlayResult, Error> {
    use std::io::Write as _;

    let mut child = std::process::Command::new("rustfmt")
        .args(&[
            "--edition",
            match edition {
                api::Edition::E2015 => "2015",
                api::Edition::E2018 => "2018",
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
    Ok(api::PlayResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

// This function must not break when provided non-formatted text with messed up formatting: rustfmt
// may not be installed on the host's computer!
pub fn strip_fn_main_boilerplate_from_formatted(text: &str) -> String {
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
pub fn format_play_eval_stderr(stderr: &str, warn: bool) -> String {
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
