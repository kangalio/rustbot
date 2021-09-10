//! Commands that execute code

pub mod godbolt;
pub mod playground;

use crate::{Error, PrefixContext};

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
async fn reply_potentially_long_text(
    ctx: PrefixContext<'_>,
    mut text_body: &str,
    text_end: &str,
    truncation_msg: &str,
) -> Result<(), Error> {
    const MAX_OUTPUT_LINES: usize = 45;

    let mut was_truncated = false;

    // check Discord's 2000 char message limit first
    if text_body.len() + text_end.len() > 2000 {
        was_truncated = true;

        // This is how long the text body may be at max to conform to Discord's limit
        let available_space = 2000_usize
            .saturating_sub(text_end.len())
            .saturating_sub(truncation_msg.len());

        let mut cut_off_point = available_space;
        while !text_body.is_char_boundary(cut_off_point) {
            cut_off_point -= 1;
        }

        text_body = &text_body[..cut_off_point];
    }

    // check number of lines
    let text_body = if text_body.lines().count() > MAX_OUTPUT_LINES {
        was_truncated = true;

        text_body
            .lines()
            .take(MAX_OUTPUT_LINES)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        text_body.to_owned()
    };

    let msg = if was_truncated {
        format!("{}{}{}", text_body, text_end, truncation_msg)
    } else {
        format!("{}{}", text_body, text_end)
    };

    poise::say_reply(ctx.into(), msg).await?;
    Ok(())
}
