//! Commands that execute code

mod godbolt;
pub use godbolt::*;

mod playground;
pub use playground::*;

use crate::{Args, Error};

/// Send a Discord reply message and truncate the message with a given truncation message if the
/// text is too long. "Too long" means, it either goes beyond Discord's 2000 char message limit,
/// or if the text_body has too many lines.
///
/// Only `text_body` is truncated. `text_end` will always be appended at the end. This is useful
/// for example for large code blocks. You will want to truncate the code block contents, but the
/// finalizing \`\`\` should always stay - that's what `text_end` is for.
///
/// ```rust,no_run
/// # let args = todo!(); use rustbot::reply_potentially_long_text;
/// // This will send "```\nvery long stringvery long stringver...long stringve\n```"
/// //                character limit reached, text_end starts ~~~~~~~~~~~~~~~~^
/// reply_potentially_long_text(
///     args,
///     format!("```\n{}", "very long string".repeat(500)),
///     "\n```"
/// );
/// ```
fn reply_potentially_long_text(
    args: &Args,
    text_body: &str,
    text_end: &str,
    truncation_msg: &str,
) -> Result<(), Error> {
    const MAX_OUTPUT_LINES: usize = 45;

    // check the 2000 char limit first, because otherwise we could produce a too large message
    let msg = if text_body.len() + text_end.len() > 2000 {
        // This is how long the text body may be at max to conform to Discord's limit
        let available_space = 2000 - text_end.len() - truncation_msg.len();

        let mut cut_off_point = available_space;
        while !text_body.is_char_boundary(cut_off_point) {
            cut_off_point -= 1;
        }

        format!(
            "{}{}{}",
            &text_body[..cut_off_point],
            text_end,
            truncation_msg
        )
    } else if text_body.lines().count() > MAX_OUTPUT_LINES {
        format!(
            "{}{}{}",
            text_body
                .lines()
                .take(MAX_OUTPUT_LINES)
                .collect::<Vec<_>>()
                .join("\n"),
            text_end,
            truncation_msg,
        )
    } else {
        format!("{}{}", text_body, text_end)
    };

    crate::send_reply(args, &msg)
}

/// Extract code from a Discord code block on a best-effort basis
///
/// ```rust
/// # use rustbot::extract_code;
/// assert_eq!(extract_code("`hello`").unwrap(), "hello");
/// assert_eq!(extract_code("`    hello `").unwrap(), "hello");
/// assert_eq!(extract_code("``` hello ```").unwrap(), "hello");
/// assert_eq!(extract_code("```rust hello ```").unwrap(), "hello");
/// assert_eq!(extract_code("```rust\nhello\n```").unwrap(), "hello");
/// assert_eq!(extract_code("``` rust\nhello\n```").unwrap(), "rust\nhello");
/// ```
fn extract_code(input: &str) -> Result<&str, Error> {
    fn inner(input: &str) -> Option<&str> {
        let input = input.trim();

        let extracted_code = if input.starts_with("```") && input.ends_with("```") {
            let code_starting_point = input.find(char::is_whitespace)?; // skip over lang specifier
            let code_end_point = input.len() - 3;

            // can't fail but you can never be too sure
            input.get(code_starting_point..code_end_point)?
        } else if input.starts_with('`') && input.ends_with('`') {
            // can't fail but you can never be too sure
            input.get(1..(input.len() - 1))?
        } else {
            return None;
        };

        Some(extracted_code.trim())
    }

    Ok(inner(input).ok_or(
        "Missing code block. Please use the following markdown:
\\`code here\\`
or
\\`\\`\\`rust
code here
\\`\\`\\`",
    )?)
}
