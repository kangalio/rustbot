#[macro_use]
extern crate log;

mod api;
mod command_history;
mod commands;
mod crates;
mod godbolt;
mod moderation;
mod playground;

use commands::{Args, Commands};
use serenity::{model::prelude::*, prelude::*};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(serde::Deserialize)]
struct Config {
    discord_token: String,
    mod_role_id: u64,
}

fn app() -> Result<(), Error> {
    let Config {
        discord_token,
        mod_role_id,
    } = envy::from_env::<Config>()?;

    info!("starting...");

    let mut cmds = Commands::new();

    cmds.add(
        "crate",
        crates::search,
        "Lookup crates on crates.io",
        crates::help,
    )
    .broadcast_typing = true;

    cmds.add(
        "docs",
        crates::doc_search,
        "Lookup documentation",
        crates::doc_help,
    )
    .broadcast_typing = true;

    cmds.add(
        "play",
        playground::play,
        "Compile and run rust code in a playground",
        |args| playground::play_and_eval_help(args, "play"),
    )
    .broadcast_typing = true;

    cmds.add(
        "eval",
        playground::eval,
        "Evaluate a single rust expression",
        |args| playground::play_and_eval_help(args, "eval"),
    )
    .broadcast_typing = true;

    cmds.add(
        "miri",
        playground::miri,
        "Run code and detect undefined behavior using Miri",
        playground::miri_help,
    )
    .broadcast_typing = true;

    cmds.add(
        "expand",
        playground::expand_macros,
        "Expand macros to their raw desugared form",
        playground::expand_macros_help,
    )
    .broadcast_typing = true;

    cmds.add(
        "clippy",
        playground::clippy,
        "Catch common mistakes using the Clippy linter",
        playground::clippy_help,
    )
    .broadcast_typing = true;

    cmds.add(
        "fmt",
        playground::fmt,
        "Format code using rustfmt",
        playground::fmt_help,
    )
    .broadcast_typing = true;

    cmds.add(
        "go",
        |args| api::send_reply(args, "No"),
        "Evaluates Go code",
        |args| api::send_reply(args, "Evaluates Go code"),
    );

    cmds.add(
        "godbolt",
        godbolt::godbolt,
        "View assembly using Godbolt",
        godbolt::help,
    )
    .broadcast_typing = true;

    cmds.add(
        "cleanup",
        move |args| moderation::cleanup(args, RoleId(mod_role_id)),
        "Deletes the bot's messages for cleanup",
        moderation::cleanup_help,
    );

    cmds.add(
        "ban",
        moderation::joke_ban,
        "Bans another person",
        moderation::joke_ban_help,
    )
    .aliases = &["banne"];

    cmds.add(
        "source",
        |args| api::send_reply(args, "https://github.com/kangalioo/rustbot"),
        "Links to the bot GitHub repo",
        |args| api::send_reply(args, "?source\n\nLinks to the bot GitHub repo"),
    );

    Client::new_with_extras(&discord_token, |e| e.event_handler(Events { cmds }))?.start()?;
    Ok(())
}

/// Send a Discord reply message and truncate the message with a given truncation message if the
/// text is too long. "Too long" means, it either goes beyond Discord's 2000 char message limit,
/// or if the text_body has too many lines.
///
/// Only `text_body` is truncated. `text_end` will always be appended at the end. This is useful
/// for example for large code blocks. You will want to truncate the code block contents, but the
/// finalizing \`\`\` should always stay - that's what `text_end` is for.
///
/// ```rust,no_run
/// # let args = todo!();
/// // This will send "```\nvery long stringvery long stringver...long stringve\n```"
/// //                Character limit reached, text_end starts ~~~~~~~~~~~~~~~~^
/// reply_potentially_long_text(
///     args,
///     format!("```\n{}", "very long string".repeat(500)),
///     "\n```"
/// )
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

    api::send_reply(args, &msg)
}

/// Extract code from a Discord code block on a best-effort basis
///
/// ```rust
/// assert_eq!(extract_code("`hello`"), Some("hello"));
/// assert_eq!(extract_code("`    hello `"), Some("hello"));
/// assert_eq!(extract_code("``` hello ```"), Some("hello"));
/// assert_eq!(extract_code("```rust hello ```"), Some("hello"));
/// assert_eq!(extract_code("```rust\nhello\n```"), Some("hello"));
/// assert_eq!(extract_code("``` rust\nhello\n```"), Some("rust\nhello"));
/// ```
pub fn extract_code(input: &str) -> Result<&str, Error> {
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

pub fn find_custom_emoji(args: &Args, emoji_name: &str) -> Option<Emoji> {
    args.msg.guild(&args.cx.cache).and_then(|guild| {
        guild
            .read()
            .emojis
            .values()
            .find(|emoji| emoji.name.eq_ignore_ascii_case(emoji_name))
            .cloned()
    })
}

pub fn custom_emoji_code(args: &Args, emoji_name: &str, fallback: char) -> String {
    match find_custom_emoji(args, emoji_name) {
        Some(emoji) => emoji.to_string(),
        None => fallback.to_string(),
    }
}

// React with a custom emoji from the guild, or fallback to a default Unicode emoji
pub fn react_custom_emoji(args: &Args, emoji_name: &str, fallback: char) -> Result<(), Error> {
    let reaction = find_custom_emoji(args, emoji_name)
        .map(ReactionType::from)
        .unwrap_or_else(|| ReactionType::from(fallback));

    args.msg.react(&args.cx.http, reaction)?;
    Ok(())
}

fn main() {
    env_logger::init();

    if let Err(e) = app() {
        error!("{}", e);
        std::process::exit(1);
    }
}

struct BotUserId;

impl TypeMapKey for BotUserId {
    type Value = UserId;
}

struct Events {
    cmds: Commands,
}

impl EventHandler for Events {
    fn ready(&self, cx: Context, ready: Ready) {
        info!("{} connected to discord", ready.user.name);
        {
            let mut data = cx.data.write();
            data.insert::<command_history::CommandHistory>(indexmap::IndexMap::new());
            data.insert::<BotUserId>(ready.user.id);
        }

        std::thread::spawn(move || -> Result<(), Error> {
            loop {
                command_history::clear_command_history(&cx)?;
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        });
    }

    fn message(&self, cx: Context, message: Message) {
        self.cmds.execute(&cx, &message);
    }

    fn message_update(
        &self,
        cx: Context,
        _: Option<Message>,
        _: Option<Message>,
        ev: MessageUpdateEvent,
    ) {
        if let Err(e) = command_history::replay_message(cx, ev, &self.cmds) {
            error!("{}", e);
        }
    }

    fn message_delete(&self, cx: Context, channel_id: ChannelId, message_id: MessageId) {
        let mut data = cx.data.write();
        let history = data.get_mut::<command_history::CommandHistory>().unwrap();
        if let Some(response_id) = history.remove(&message_id) {
            info!("deleting message: {:?}", response_id);
            let _ = channel_id.delete_message(&cx, response_id);
        }
    }
}
