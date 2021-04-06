mod code_execution;
mod crates;
mod misc;
mod moderation;

use serenity::model::prelude::*;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

// Wraps a command function and reacts with a red cross emoji on error
async fn react_cross(error: Error, ctx: poise::CommandErrorContext<'_, Data, Error>) {
    println!("Reacting with red cross because of error: {}", error);
    if let Err(e) = ctx
        .ctx
        .msg
        .react(ctx.ctx.discord, ReactionType::from('❌'))
        .await
    {
        println!("Failed to react with red cross: {}", e);
    }
}

async fn on_error(error: Error, ctx: poise::ErrorContext<'_, Data, Error>) {
    if let poise::ErrorContext::Command(ctx) = ctx {
        let reply = if let Some(poise::ArgumentParseError(error)) = error.downcast_ref() {
            if error.is::<poise::CodeBlockError>() {
                "Missing code block. Please use the following markdown:
\\`code here\\`
or
\\`\\`\\`rust
code here
\\`\\`\\`"
                    .to_owned()
            } else if let Some(explanation) = &ctx.command.options.explanation {
                format!("**{}**\n{}", error, explanation())
            } else {
                error.to_string()
            }
        } else {
            error.to_string()
        };
        if let Err(e) = poise::say_reply(ctx.ctx, reply).await {
            log::warn!("{}", e);
        }
    }
}

pub struct Data {
    bot_user_id: UserId,
    mod_role_id: RoleId,
    rustacean_role: RoleId,
    http: reqwest::Client,
}

async fn app() -> Result<(), Error> {
    let discord_token = std::env::var("DISCORD_TOKEN").map_err(|_| "Missing DISCORD_TOKEN")?;
    let mod_role_id = std::env::var("MOD_ROLE_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or("Missing/invalid MOD_ROLE_ID")?;
    let rustacean_role = std::env::var("RUSTACEAN_ROLE_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or("Missing/invalid RUSTACEAN_ROLE_ID")?;

    let commands = vec![
        misc::help(),
        crates::crate_(),
        crates::doc(),
        code_execution::play(),
        code_execution::eval(),
        code_execution::miri(),
        code_execution::expand(),
        code_execution::clippy(),
        code_execution::fmt(),
        code_execution::microbench(),
        code_execution::procmacro(),
        misc::go(),
        code_execution::godbolt(),
        moderation::cleanup(),
        moderation::ban(),
        moderation::rustify(),
        misc::source(),
    ];

    let framework = poise::Framework::new(
        "?",
        move |_, bot| Data {
            bot_user_id: bot.user.id,
            mod_role_id,
            rustacean_role,
            http: reqwest::Client::new(),
        },
        poise::FrameworkOptions {
            commands,
            additional_prefixes: &[
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
            ],
            edit_tracker: Some(poise::EditTracker::for_timespan(
                std::time::Duration::from_secs(3600),
            )),
            on_error: |error, ctx| Box::pin(on_error(error, ctx)),
            ..Default::default()
        },
    );

    framework.start(&discord_token).await?;
    Ok(())
}

pub async fn find_custom_emoji(ctx: Context<'_>, emoji_name: &str) -> Option<Emoji> {
    ctx.msg.guild(ctx.discord).await.and_then(|guild| {
        guild
            .emojis
            .values()
            .find(|emoji| emoji.name.eq_ignore_ascii_case(emoji_name))
            .cloned()
    })
}

pub async fn custom_emoji_code(ctx: Context<'_>, emoji_name: &str, fallback: char) -> String {
    match find_custom_emoji(ctx, emoji_name).await {
        Some(emoji) => emoji.to_string(),
        None => fallback.to_string(),
    }
}

// React with a custom emoji from the guild, or fallback to a default Unicode emoji
pub async fn react_custom_emoji(
    ctx: Context<'_>,
    emoji_name: &str,
    fallback: char,
) -> Result<(), Error> {
    let reaction = find_custom_emoji(ctx, emoji_name)
        .await
        .map(ReactionType::from)
        .unwrap_or_else(|| ReactionType::from(fallback));

    ctx.msg.react(ctx.discord, reaction).await?;
    Ok(())
}

pub async fn main() {
    env_logger::init();

    if let Err(e) = app().await {
        log::error!("{}", e);
        std::process::exit(1);
    }
}
