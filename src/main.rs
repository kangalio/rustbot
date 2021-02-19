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

#[tokio::main]
async fn app() -> Result<(), Error> {
    let Config {
        discord_token,
        mod_role_id,
    } = envy::from_env::<Config>()?;

    info!("starting...");

    let mut cmds = Commands::new();

    cmds.add(
        "crate",
        |args| Box::pin(crates::search(args)),
        "Lookup crates on crates.io",
        |args| Box::pin(crates::help(args)),
    )
    .broadcast_typing = true;

    cmds.add(
        "docs",
        |args| Box::pin(crates::doc_search(args)),
        "Lookup documentation",
        |args| Box::pin(crates::doc_help(args)),
    )
    .broadcast_typing = true;

    cmds.add(
        "play",
        |args| Box::pin(playground::play(args)),
        "Compile and run rust code in a playground",
        |args| Box::pin(playground::play_and_eval_help(args, "play")),
    )
    .broadcast_typing = true;

    cmds.add(
        "eval",
        |args| Box::pin(playground::eval(args)),
        "Evaluate a single rust expression",
        |args| Box::pin(playground::play_and_eval_help(args, "eval")),
    )
    .broadcast_typing = true;

    cmds.add(
        "miri",
        |args| Box::pin(playground::miri(args)),
        "Run code and detect undefined behavior using Miri",
        |args| Box::pin(playground::miri_help(args)),
    )
    .broadcast_typing = true;

    cmds.add(
        "expand",
        |args| Box::pin(playground::expand_macros(args)),
        "Expand macros to their raw desugared form",
        |args| Box::pin(playground::expand_macros_help(args)),
    )
    .broadcast_typing = true;

    cmds.add(
        "clippy",
        |args| Box::pin(playground::clippy(args)),
        "Catch common mistakes using the Clippy linter",
        |args| Box::pin(playground::clippy_help(args)),
    )
    .broadcast_typing = true;

    cmds.add(
        "fmt",
        |args| Box::pin(playground::fmt(args)),
        "Format code using rustfmt",
        |args| Box::pin(playground::fmt_help(args)),
    )
    .broadcast_typing = true;

    cmds.add(
        "go",
        |args| Box::pin(api::send_reply(args, "No")),
        "Evaluates Go code",
        |args| Box::pin(api::send_reply(args, "Evaluates Go code")),
    );

    cmds.add(
        "godbolt",
        |args| Box::pin(godbolt::godbolt(args)),
        "View assembly using Godbolt",
        |args| Box::pin(godbolt::help(args)),
    )
    .broadcast_typing = true;

    cmds.add(
        "cleanup",
        move |args| Box::pin(moderation::cleanup(args, RoleId(mod_role_id))),
        "Deletes the bot's messages for cleanup",
        |args| Box::pin(moderation::cleanup_help(args)),
    );

    cmds.add(
        "ban",
        |args| Box::pin(moderation::joke_ban(args)),
        "Bans another person",
        |args| Box::pin(moderation::joke_ban_help(args)),
    )
    .aliases = &["banne"];

    cmds.add(
        "source",
        |args| {
            Box::pin(api::send_reply(
                args,
                "https://github.com/kangalioo/discord-mods-bot",
            ))
        },
        "Links to the bot GitHub repo",
        |args| {
            Box::pin(api::send_reply(
                args,
                "?source\n\nLinks to the bot GitHub repo",
            ))
        },
    );

    Client::builder(&discord_token)
        .event_handler(Events { cmds })
        .await?
        .start()
        .await?;
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
async fn reply_potentially_long_text(
    args: &Args<'_>,
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

    api::send_reply(args, &msg).await
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

pub async fn find_custom_emoji(args: &Args<'_>, emoji_name: &str) -> Option<Emoji> {
    args.msg.guild(&args.cx.cache).await.and_then(|guild| {
        guild
            .emojis
            .values()
            .find(|emoji| emoji.name.eq_ignore_ascii_case(emoji_name))
            .cloned()
    })
}

pub async fn custom_emoji_code(args: &Args<'_>, emoji_name: &str, fallback: char) -> String {
    match find_custom_emoji(args, emoji_name).await {
        Some(emoji) => emoji.to_string(),
        None => fallback.to_string(),
    }
}

// React with a custom emoji from the guild, or fallback to a default Unicode emoji
pub async fn react_custom_emoji(
    args: &Args<'_>,
    emoji_name: &str,
    fallback: char,
) -> Result<(), Error> {
    let reaction = find_custom_emoji(args, emoji_name)
        .await
        .map(ReactionType::from)
        .unwrap_or_else(|| ReactionType::from(fallback));

    args.msg.react(&args.cx.http, reaction).await?;
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

#[async_trait::async_trait]
impl EventHandler for Events {
    async fn ready(&self, cx: Context, ready: Ready) {
        info!("{} connected to discord", ready.user.name);
        {
            let mut data = cx.data.write().await;
            data.insert::<command_history::CommandHistory>(indexmap::IndexMap::new());
            data.insert::<BotUserId>(ready.user.id);
        }

        tokio::spawn(async move {
            loop {
                command_history::clear_command_history(&cx).await;
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        });
    }

    async fn message(&self, cx: Context, message: Message) {
        self.cmds.execute(&cx, &message).await;
    }

    async fn message_update(
        &self,
        cx: Context,
        _: Option<Message>,
        _: Option<Message>,
        ev: MessageUpdateEvent,
    ) {
        if let Err(e) = command_history::replay_message(cx, ev, &self.cmds).await {
            error!("{}", e);
        }
    }

    async fn message_delete(
        &self,
        cx: Context,
        channel_id: ChannelId,
        message_id: MessageId,
        _guild_id: Option<GuildId>,
    ) {
        let mut data = cx.data.write().await;
        let history = data.get_mut::<command_history::CommandHistory>().unwrap();
        if let Some(response_id) = history.remove(&message_id) {
            info!("deleting message: {:?}", response_id);
            let _ = channel_id.delete_message(&cx, response_id);
        }
    }
}
