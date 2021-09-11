use crate::{Context, Error, PrefixContext};

/// Evaluates Go code
#[poise::command(prefix_command, discard_spare_arguments)]
pub async fn go(ctx: PrefixContext<'_>) -> Result<(), Error> {
    poise::say_reply(ctx.into(), "No").await?;
    Ok(())
}

/// Links to the bot GitHub repo
#[poise::command(prefix_command, discard_spare_arguments, slash_command)]
pub async fn source(ctx: Context<'_>) -> Result<(), Error> {
    poise::say_reply(ctx, r"https://github.com/kangalioo/rustbot").await?;
    Ok(())
}

/// Show this menu
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"] command: Option<String>,
) -> Result<(), Error> {
    let bottom_text = "You can still use all commands with `?`, even if it says `/` above.
Type ?help command for more info on a command.
You can edit your message to the bot and the bot will edit its response.";
    poise::defaults::help(
        ctx,
        command.as_deref(),
        bottom_text,
        poise::defaults::HelpResponseMode::Ephemeral,
    )
    .await?;
    Ok(())
}

/// Register slash commands in this guild or globally
///
/// Run with no arguments to register in guild, run with argument "global" to register globally.
#[poise::command(prefix_command, hide_in_help)]
pub async fn register(ctx: PrefixContext<'_>, #[flag] global: bool) -> Result<(), Error> {
    poise::defaults::register_application_commands(ctx.into(), global).await?;

    Ok(())
}

/// Tells you how long the bot has been up for
#[poise::command(prefix_command, slash_command, hide_in_help)]
pub async fn uptime(ctx: Context<'_>) -> Result<(), Error> {
    let uptime = std::time::Instant::now() - ctx.data().bot_start_time;

    let div_mod = |a, b| (a / b, a % b);

    let seconds = uptime.as_secs();
    let (minutes, seconds) = div_mod(seconds, 60);
    let (hours, minutes) = div_mod(minutes, 60);
    let (days, hours) = div_mod(hours, 24);

    poise::say_reply(
        ctx,
        format!("Uptime: {}d {}h {}m {}s", days, hours, minutes, seconds),
    )
    .await?;

    Ok(())
}

/// List servers of which the bot is a member of
#[poise::command(slash_command, prefix_command, track_edits, hide_in_help)]
pub async fn servers(ctx: Context<'_>) -> Result<(), Error> {
    let mut show_private_guilds = false;
    if let Context::Application(_) = ctx {
        if let Ok(app) = ctx.discord().http.get_current_application_info().await {
            if app.owner.id == ctx.author().id {
                show_private_guilds = true;
            }
        }
    }

    struct Guild {
        name: String,
        num_members: u64,
        is_public: bool,
    }

    let guild_ids = ctx.discord().cache.guilds();
    let mut guilds = guild_ids
        .into_iter()
        .filter_map(|guild_id| {
            ctx.discord().cache.guild_field(guild_id, |guild| Guild {
                name: guild.name.clone(),
                num_members: guild.member_count,
                is_public: guild.features.iter().any(|x| x == "DISCOVERABLE"),
            })
        })
        .collect::<Vec<_>>();
    guilds.sort_by_key(|guild| u64::MAX - guild.num_members); // sort descending

    let mut num_private_guilds = 0;
    let mut num_private_guild_members = 0;
    let mut response = format!("I am currently in {} servers!\n", guilds.len());
    for guild in guilds {
        if guild.is_public || show_private_guilds {
            response += &format!("- **{}** ({} members)\n", guild.name, guild.num_members);
        } else {
            num_private_guilds += 1;
            num_private_guild_members += guild.num_members;
        }
    }
    if num_private_guilds > 0 {
        response += &format!(
            "- [{} private servers with {} members total]\n",
            num_private_guilds, num_private_guild_members
        );
    }

    if show_private_guilds {
        response += "\n_Showing private guilds because you are the bot owner_";
    }

    poise::send_reply(ctx, |f| f.content(response).ephemeral(show_private_guilds)).await?;

    Ok(())
}
