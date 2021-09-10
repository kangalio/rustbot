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

#[poise::command(prefix_command, track_edits, hide_in_help)]
pub async fn servers(ctx: Context<'_>) -> Result<(), Error> {
    let current_user = ctx.discord().http.get_current_user().await?;
    let guilds = current_user.guilds(ctx.discord()).await?;

    let mut response = format!("I am currently in {} servers!\n", guilds.len());
    for guild in guilds {
        response += &format!("- {}\n", guild.name);
    }

    poise::say_reply(ctx, response).await?;

    Ok(())
}
