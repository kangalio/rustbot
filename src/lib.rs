mod code_execution;
mod crates;
mod misc;
mod moderation;

use poise::serenity_prelude as serenity;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type PrefixContext<'a> = poise::PrefixContext<'a, Data, Error>;
pub type SlashContext<'a> = poise::SlashContext<'a, Data, Error>;

// pub const EMBED_COLOR: (u8, u8, u8) = (0xf7, 0x4c, 0x00);
pub const EMBED_COLOR: (u8, u8, u8) = (0xb7, 0x47, 0x00); // slightly less saturated

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

async fn acknowledge_prefix_fail(
    error: Error,
    ctx: poise::PrefixCommandErrorContext<'_, Data, Error>,
) {
    acknowledge_fail(error, poise::CommandErrorContext::Prefix(ctx)).await
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
    reports_channel: Option<serenity::ChannelId>,
    bot_start_time: std::time::Instant,
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
    let reports_channel = env_var("REPORTS_CHANNEL_ID").ok();
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
                "yo crab can you ",
                "yo crab, can you ",
            ],
            edit_tracker: Some(poise::EditTracker::for_timespan(
                std::time::Duration::from_secs(3600 * 24 * 2),
            )),
            ..Default::default()
        },
        pre_command: |ctx| {
            Box::pin(async move {
                let datetime = ctx.created_at();
                let channel_name = ctx
                    .channel_id()
                    .name(&ctx.discord())
                    .await
                    .unwrap_or_else(|| "<unknown>".to_owned());
                let author = ctx.author().tag();

                match ctx {
                    poise::Context::Prefix(ctx) => {
                        println!(
                            "[{}] {} in {}: {}",
                            datetime, author, channel_name, &ctx.msg.content
                        );
                    }
                    poise::Context::Slash(ctx) => {
                        let command_name = &ctx.interaction.data.name;

                        println!(
                            "[{}] {} in {} used slash command '{}'",
                            datetime, author, channel_name, command_name
                        );
                    }
                }
            })
        },
        on_error: |error, ctx| Box::pin(on_error(error, ctx)),
        ..Default::default()
    };

    options.command(code_execution::play(), |f| f.category("Playground"));
    options.command(code_execution::playwarn(), |f| f.category("Playground"));
    options.command(code_execution::eval(), |f| f.category("Playground"));
    options.command(code_execution::miri(), |f| f.category("Playground"));
    options.command(code_execution::expand(), |f| f.category("Playground"));
    options.command(code_execution::clippy(), |f| f.category("Playground"));
    options.command(code_execution::fmt(), |f| f.category("Playground"));
    options.command(code_execution::microbench(), |f| f.category("Playground"));
    options.command(code_execution::procmacro(), |f| f.category("Playground"));
    options.command(code_execution::godbolt(), |f| f.category("Playground"));
    options.command(code_execution::mca(), |f| f.category("Playground"));
    options.command(code_execution::llvmir(), |f| f.category("Playground"));
    options.command(code_execution::asmdiff(), |f| f.category("Playground"));
    options.command(crates::crate_(), |f| f.category("Crates"));
    options.command(crates::doc(), |f| f.category("Crates"));
    options.command(moderation::cleanup(), |f| f.category("Moderation"));
    options.command(moderation::ban(), |f| f.category("Moderation"));
    options.command(moderation::move_(), |f| f.category("Moderation"));
    options.command(misc::go(), |f| f.category("Miscellaneous"));
    options.command(misc::source(), |f| f.category("Miscellaneous"));
    options.command(misc::help(), |f| f.category("Miscellaneous"));
    options.command(misc::register(), |f| f.category("Miscellaneous"));
    options.command(misc::uptime(), |f| f.category("Miscellaneous"));

    // Use different implementations for prefix and slash version of rustify
    let prefix_impl = moderation::prefix_rustify().0;
    let slash_impl = moderation::slash_rustify().1;
    options.command((prefix_impl, slash_impl), |f| f.category("Moderation"));

    if reports_channel.is_some() {
        options.command(moderation::report(), |f| f.category("Moderation"));
    }

    let framework = poise::Framework::new(
        "?",
        serenity::ApplicationId(application_id),
        move |ctx, bot, _framework| {
            Box::pin(async move {
                ctx.set_activity(serenity::Activity::listening("?help"))
                    .await;
                Ok(Data {
                    bot_user_id: bot.user.id,
                    mod_role_id,
                    rustacean_role,
                    reports_channel,
                    bot_start_time: std::time::Instant::now(),
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
                .intents(
                    serenity::GatewayIntents::non_privileged()
                        | serenity::GatewayIntents::GUILD_MEMBERS,
                ),
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
                let message_we_just_sent = ctx
                    .interaction
                    .channel_id
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
    Ok(())
}

pub async fn main() {
    env_logger::init();

    if let Err(e) = app().await {
        log::error!("{}", e);
        std::process::exit(1);
    }
}
