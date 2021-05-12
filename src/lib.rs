mod code_execution;
mod crates;
mod misc;
mod moderation;

use poise::serenity_prelude as serenity;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type PrefixContext<'a> = poise::PrefixContext<'a, Data, Error>;
pub type SlashContext<'a> = poise::SlashContext<'a, Data, Error>;

/// In prefix commands, react with a red cross emoji. In slash commands, respond with a short
/// explanation.
async fn acknowledge_fail(error: Error, ctx: poise::CommandErrorContext<'_, Data, Error>) {
    println!("Reacting with red cross because of error: {}", error);
    match ctx {
        poise::CommandErrorContext::Prefix(ctx) => {
            if let Err(e) = ctx
                .ctx
                .msg
                .react(ctx.ctx.discord, serenity::ReactionType::from('❌'))
                .await
            {
                println!("Failed to react with red cross: {}", e);
            }
        }
        poise::CommandErrorContext::Slash(ctx) => {
            if let Err(e) = poise::say_slash_reply(ctx.ctx, format!("❌ {}", error)).await {
                println!(
                    "Failed to send failure acknowledgment slash command response: {}",
                    e
                );
            }
        }
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
            } else if let poise::CommandErrorContext::Prefix(poise::PrefixCommandErrorContext {
                command:
                    poise::PrefixCommand {
                        options:
                            poise::PrefixCommandOptions {
                                multiline_help: Some(multiline_help),
                                ..
                            },
                        ..
                    },
                ..
            }) = ctx
            {
                format!("**{}**\n{}", error, multiline_help())
            } else {
                error.to_string()
            }
        } else {
            error.to_string()
        };
        if let Err(e) = poise::say_reply(ctx.ctx(), reply).await {
            log::warn!("{}", e);
        }
    }
}

pub struct Data {
    bot_user_id: serenity::UserId,
    #[allow(dead_code)] // might add back in
    mod_role_id: serenity::RoleId,
    rustacean_role: serenity::RoleId,
    http: reqwest::Client,
}

fn env_var<T: std::str::FromStr>(name: &str) -> Result<T, Error>
where
    T::Err: std::fmt::Display,
{
    Ok(std::env::var(name)
        .map_err(|_| format!("Missing {}", name))?
        .parse()
        .map_err(|e| format!("Invalid {}: {}", name, e))?)
}

async fn app() -> Result<(), Error> {
    let discord_token: String = env_var("DISCORD_TOKEN")?;
    let mod_role_id = env_var("MOD_ROLE_ID")?;
    let rustacean_role = env_var("RUSTACEAN_ROLE_ID")?;
    let application_id = env_var("APPLICATION_ID")?;

    let mut options = poise::FrameworkOptions {
        prefix_options: poise::PrefixFrameworkOptions {
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
            ..Default::default()
        },
        on_error: |error, ctx| Box::pin(on_error(error, ctx)),
        ..Default::default()
    };
    options.command(misc::help);
    options.command(crates::crate_);
    options.command(crates::doc);
    options.command(code_execution::play);
    options.command(code_execution::eval);
    options.command(code_execution::miri);
    options.command(code_execution::expand);
    options.command(code_execution::clippy);
    options.command(code_execution::fmt);
    options.command(code_execution::microbench);
    options.command(code_execution::procmacro);
    options.command(misc::go);
    options.command(code_execution::godbolt);
    options.command(moderation::cleanup);
    options.command(moderation::ban);
    options.command(moderation::rustify);
    options.command(misc::about);
    options.command(misc::register);

    let framework = poise::Framework::new(
        "?",
        serenity::ApplicationId(application_id),
        move |_ctx, bot, _framework| {
            Box::pin(async move {
                Ok(Data {
                    bot_user_id: bot.user.id,
                    mod_role_id,
                    rustacean_role,
                    http: reqwest::Client::new(),
                })
            })
        },
        options,
    );

    framework
        .start(
            serenity::ClientBuilder::new(discord_token)
                .application_id(application_id)
                .intents(serenity::GatewayIntents::all()),
        )
        .await?;
    Ok(())
}

pub async fn find_custom_emoji(ctx: Context<'_>, emoji_name: &str) -> Option<serenity::Emoji> {
    ctx.guild_id()?
        .to_guild_cached(ctx.discord())
        .await?
        .emojis
        .values()
        .find(|emoji| emoji.name.eq_ignore_ascii_case(emoji_name))
        .cloned()
}

pub async fn custom_emoji_code(ctx: Context<'_>, emoji_name: &str, fallback: char) -> String {
    match find_custom_emoji(ctx, emoji_name).await {
        Some(emoji) => emoji.to_string(),
        None => fallback.to_string(),
    }
}

/// In prefix commands, react with a custom emoji from the guild, or fallback to a default Unicode
/// emoji.
///
/// In slash commands, currently nothing happens.
pub async fn acknowledge_success(
    ctx: Context<'_>,
    emoji_name: &str,
    fallback: char,
) -> Result<(), Error> {
    let emoji = find_custom_emoji(ctx, emoji_name).await;
    match ctx {
        Context::Prefix(ctx) => {
            let reaction = emoji
                .map(serenity::ReactionType::from)
                .unwrap_or_else(|| serenity::ReactionType::from(fallback));

            ctx.msg.react(ctx.discord, reaction).await?;
        }
        Context::Slash(ctx) => {
            // this is a bad solution........ it will attempt to acknowledge the user command
            // but ignore failures because a response might have been sent already
            let msg_content = match emoji {
                Some(e) => e.to_string(),
                None => fallback.to_string(),
            };
            if let Ok(()) = poise::say_slash_reply(ctx, msg_content.clone()).await {
                if let Some(channel) = ctx.interaction.channel_id {
                    let message_we_just_sent = channel
                        .messages(ctx.discord, |f| f.limit(10))
                        .await?
                        .into_iter()
                        .find(|msg| msg.content == msg_content);
                    if let Some(message_we_just_sent) = message_we_just_sent {
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        message_we_just_sent.delete(ctx.discord).await?;
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn main() {
    env_logger::init();

    if let Err(e) = app().await {
        log::error!("{}", e);
        std::process::exit(1);
    }
}
