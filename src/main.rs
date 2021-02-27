#![allow(unused)] // temporary

#[macro_use]
extern crate log;

mod api;
mod command_history;
mod crates;
mod godbolt;
mod moderation;
mod playground;

use api::send_reply;
use serenity::{
    client::bridge::gateway::GatewayIntents, futures::lock::Mutex, model::prelude::*, prelude::*,
};
use serenity_framework::prelude::*;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context = FrameworkContext<Data>;

const PREFIXES: &[&str] = &[
    "?",
    "🦀 ",
    "🦀",
    "<:ferris:358652670585733120> ",
    "<:ferris:358652670585733120>",
    "hey ferris can you please ",
    "hey ferris, can you please ",
    "hey fewwis can you please ",
    "hey fewwis, can you please ",
    "hey ferris can you ",
    "hey ferris, can you ",
    "hey fewwis can you ",
    "hey fewwis, can you ",
];

#[derive(serde::Deserialize)]
struct Config {
    discord_token: String,
    mod_role_id: u64,
}

/// Data is accessible to every command function
pub struct Data {
    bot_user_id: Mutex<UserId>,
    mod_role_id: RoleId,
    reqwest: reqwest::Client,
    command_history: Mutex<indexmap::IndexMap<MessageId, MessageId>>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    let Config {
        discord_token,
        mod_role_id,
    } = envy::from_env::<Config>()?;

    /*let mut cmds = Commands::new();

    cmds.add(
        "crate",
        |args| Box::pin(crates::crate_(args)),
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
    );*/

    // can't use struct expression because non_exhaustive
    let mut framework = Configuration::<Data>::default();
    framework.prefixes = PREFIXES.iter().map(|&p| p.to_owned()).collect();
    framework.case_insensitive = true;

    for &cmd in &[
        moderation::ban,
        moderation::cleanup,
        crates::crate_,
        crates::docs,
        playground::play,
        playground::eval,
        playground::miri,
        playground::expand_macros,
        playground::clippy,
        playground::fmt,
    ] {
        framework.command(cmd);
    }

    Client::builder(&discord_token)
        .intents(GatewayIntents::all()) // Quick and easy solution. It won't hurt, right..?
        .event_handler(Events {
            framework: serenity_framework::Framework::with_data(
                framework,
                Data {
                    reqwest: reqwest::Client::new(),
                    bot_user_id: Mutex::new(UserId(0)),
                    mod_role_id: RoleId(mod_role_id),
                    command_history: Default::default(),
                },
            ),
        })
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
    ctx: &Context,
    msg: &Message,
    text_body: &str,
    text_end: &str,
    truncation_msg: &str,
) -> Result<(), Error> {
    const MAX_OUTPUT_LINES: usize = 45;

    // check the 2000 char limit first, because otherwise we could produce a too large message
    let response = if text_body.len() + text_end.len() > 2000 {
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

    api::send_reply(ctx, msg, &response).await
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

pub fn find_custom_emoji(guild: Option<Guild>, emoji_name: &str) -> Option<Emoji> {
    guild?
        .emojis
        .values()
        .find(|emoji| emoji.name.eq_ignore_ascii_case(emoji_name))
        .cloned()
}

pub fn custom_emoji_code(guild: Option<Guild>, emoji_name: &str, fallback: char) -> String {
    match find_custom_emoji(guild, emoji_name) {
        Some(emoji) => emoji.to_string(),
        None => fallback.to_string(),
    }
}

// React with a custom emoji from the guild, or fallback to a default Unicode emoji
pub async fn react_custom_emoji(
    ctx: &Context,
    msg: &Message,
    emoji_name: &str,
    fallback: char,
) -> Result<(), Error> {
    let reaction = find_custom_emoji(msg.guild(&ctx.serenity_ctx.cache).await, emoji_name)
        .map(ReactionType::from)
        .unwrap_or_else(|| ReactionType::from(fallback));

    msg.react(&ctx.serenity_ctx.http, reaction).await?;
    Ok(())
}

struct BotUserId;

impl TypeMapKey for BotUserId {
    type Value = UserId;
}

pub struct Events {
    framework: serenity_framework::Framework<Data>,
}

#[async_trait::async_trait]
impl EventHandler for Events {
    async fn ready(&self, ctx: serenity::prelude::Context, ready: Ready) {
        info!("{} connected to discord", ready.user.name);
        *self.framework.data.bot_user_id.lock().await = ready.user.id;

        let data = std::sync::Arc::clone(&self.framework.data);
        tokio::spawn(async move {
            loop {
                command_history::clear_command_history(&data).await;
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        });
    }

    async fn message(&self, ctx: serenity::prelude::Context, message: Message) {
        match dbg!(self.framework.dispatch(&ctx, &message).await) {
            Ok(()) => {}
            Err(FrameworkError::User(e)) => {
                if let Err(e) =
                    api::send_reply((&ctx, &*self.framework.data), &message, &e.to_string()).await
                {
                    println!("Couldn't send error message: {}", e);
                }
            }
            Err(FrameworkError::Dispatch(e)) => match e {
                DispatchError::PrefixOnly(_)
                | DispatchError::NormalMessage
                | DispatchError::InvalidCommandName(_) => {}
                DispatchError::CheckFailed(check_name, reason) => {
                    if let Err(e) = api::send_reply(
                        (&ctx, &*self.framework.data),
                        &message,
                        &format!(
                        "Check failed... I don't know what that means? (check = {:?}, reason = {}",
                        check_name, reason
                    ),
                    )
                    .await
                    {
                        println!("Couldn't send check-failed message: {}", e);
                    }
                }
            },
            Err(FrameworkError::Dispatch(_)) => {
                if let Err(e) = message.react(ctx, '❌').await {
                    println!("Couldn't send reaction: {}", e);
                }
            }
        }
    }

    async fn message_update(
        &self,
        ctx: serenity::prelude::Context,
        _: Option<Message>,
        _: Option<Message>,
        ev: MessageUpdateEvent,
    ) {
        if let Err(e) = command_history::replay_message(ctx, ev, &self).await {
            error!("{}", e);
        }
    }

    async fn message_delete(
        &self,
        ctx: serenity::prelude::Context,
        channel_id: ChannelId,
        message_id: MessageId,
        _guild_id: Option<GuildId>,
    ) {
        let mut history = self.framework.data.command_history.lock().await;
        if let Some(response_id) = history.remove(&message_id) {
            info!("deleting message: {:?}", response_id);
            let _ = channel_id.delete_message(&ctx, response_id);
        }
    }
}
