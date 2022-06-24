use super::api;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;

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
pub fn parse_flags(mut args: poise::KeyValueArgs) -> (api::CommandFlags, String) {
    let mut errors = String::new();

    let mut flags = api::CommandFlags {
        channel: api::Channel::Nightly,
        mode: api::Mode::Debug,
        edition: api::Edition::E2021,
        warn: false,
        run: false,
    };

    macro_rules! pop_flag {
        ($flag_name:literal, $flag_field:expr) => {
            if let Some(flag) = args.0.remove($flag_name) {
                match flag.parse() {
                    Ok(x) => $flag_field = x,
                    Err(e) => errors += &format!("{}\n", e),
                }
            }
        };
    }

    pop_flag!("channel", flags.channel);
    pop_flag!("mode", flags.mode);
    pop_flag!("edition", flags.edition);
    pop_flag!("warn", flags.warn);
    pop_flag!("run", flags.run);

    for (remaining_flag, _) in args.0 {
        errors += &format!("unknown flag `{}`\n", remaining_flag);
    }

    (flags, errors)
}

pub struct GenericHelp<'a> {
    pub command: &'a str,
    pub desc: &'a str,
    pub mode_and_channel: bool,
    pub warn: bool,
    pub run: bool,
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
    if spec.run {
        reply += " run={}";
    }
    reply += " ``\u{200B}`";
    reply += spec.example_code;
    reply += "``\u{200B}`\n```\n";

    reply += "Optional arguments:\n";
    if spec.mode_and_channel {
        reply += "- mode: debug, release (default: debug)\n";
        reply += "- channel: stable, beta, nightly (default: nightly)\n";
    }
    reply += "- edition: 2015, 2018, 2021 (default: 2021)\n";
    if spec.warn {
        reply += "- warn: true, false (default: false)\n";
    }
    if spec.run {
        reply += "- run: true, false (default: false)\n";
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
    if code.contains("fn main") || code.contains("#![no_main]") {
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
    ctx: Context<'_>,
    result: api::PlayResult,
    code: &str,
    flags: &api::CommandFlags,
    flag_parse_errors: &str,
) -> Result<(), Error> {
    let result = if result.stderr.is_empty() {
        result.stdout
    } else if result.stdout.is_empty() {
        result.stderr
    } else {
        format!("{}\n{}", result.stderr, result.stdout)
    };

    // Discord displays empty code blocks weirdly if they're not formatted in a specific style,
    // so we special-case empty code blocks
    if result.trim().is_empty() {
        ctx.say(format!("{}``` ```", flag_parse_errors)).await?;
        return Ok(());
    }

    let timeout = result.contains("Killed                  timeout --signal=KILL");

    let mut text_end = String::from("```");
    if timeout {
        text_end += "Playground timeout detected";
    }

    let text = crate::trim_text(
        &format!("{}```rust\n{}", flag_parse_errors, result),
        &text_end,
        async {
            format!(
                "Output too large. Playground link: <{}>",
                api::url_from_gist(flags, &api::post_gist(ctx, code).await.unwrap_or_default()),
            )
        },
    )
    .await;

    let custom_button_id = ctx.id().to_string();
    let mut response = ctx
        .send(|b| {
            if timeout {
                b.components(|b| {
                    b.create_action_row(|b| {
                        b.create_button(|b| {
                            b.label("Retry")
                                .style(serenity::ButtonStyle::Primary)
                                .custom_id(&custom_button_id)
                        })
                    })
                });
            }
            b.content(text)
        })
        .await?
        .message()
        .await?;
    if let Some(retry_pressed) = response
        .await_component_interaction(&ctx.discord().shard)
        .filter(move |x| x.data.custom_id == custom_button_id)
        .timeout(std::time::Duration::from_secs(600))
        .await
    {
        retry_pressed
            .create_interaction_response(ctx.discord(), |b| {
                // b.kind(serenity::InteractionResponseType::Pong)
                b.kind(serenity::InteractionResponseType::DeferredUpdateMessage)
            })
            .await?;
        ctx.rerun().await?;
    } else {
        // If timed out, just remove the button
        response
            .edit(ctx.discord(), |b| b.components(|b| b))
            .await?;
    }

    Ok(())
}

// This function must not break when provided non-formatted text with messed up formatting: rustfmt
// may not be installed on the host's computer!
pub fn strip_fn_main_boilerplate_from_formatted(text: &str) -> String {
    // Remove the fn main boilerplate
    let prefix = "fn main() {";
    let postfix = "}";

    let text = match (text.find(prefix), text.rfind(postfix)) {
        (Some(prefix_pos), Some(postfix_pos)) => text
            .get((prefix_pos + prefix.len())..postfix_pos)
            .unwrap_or(text),
        _ => text,
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

/// Split stderr into compiler output and program stderr output and format the two nicely
///
/// If the program doesn't compile, the compiler output is returned. If it did compile and run,
/// compiler output (i.e. warnings) is shown only when show_compiler_warnings is true.
pub fn format_play_eval_stderr(stderr: &str, show_compiler_warnings: bool) -> String {
    let compiler_output = extract_relevant_lines(
        stderr,
        &["Compiling playground"],
        &[
            "warning emitted",
            "warnings emitted",
            "warning: `playground` (bin \"playground\") generated",
            "error: could not compile",
            "error: aborting",
            "Finished ",
        ],
    );

    if stderr.contains("Running `target") {
        // Program successfully compiled, so compiler output will be just warnings
        let program_stderr = extract_relevant_lines(stderr, &["Running `target"], &[]);

        if show_compiler_warnings {
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

pub fn stub_message(ctx: Context<'_>) -> String {
    let mut stub_message = String::from("_Running code on playground..._\n");

    if let Context::Prefix(ctx) = ctx {
        if let Some(edit_tracker) = &ctx.framework.options().prefix_options.edit_tracker {
            if let Some(existing_response) =
                edit_tracker.read().unwrap().find_bot_response(ctx.msg.id)
            {
                stub_message += &existing_response.content;
            }
        }
    }

    stub_message.truncate(2000);
    stub_message
}
