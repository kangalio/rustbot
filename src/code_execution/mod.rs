//! Commands that execute code

mod godbolt;
pub use godbolt::*;

mod playground;
pub use playground::*;

use crate::{Context, Error};

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
    ctx: Context<'_>,
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

    poise::say_reply(ctx, msg).await?;
    Ok(())
}
