use crate::{Context, Error, PrefixContext};

/// Evaluates Go code
#[poise::command(discard_spare_arguments)]
pub async fn go(ctx: PrefixContext<'_>) -> Result<(), Error> {
    poise::say_prefix_reply(ctx, "No".into()).await?;
    Ok(())
}

/// Links to the bot GitHub repo
#[poise::command(discard_spare_arguments, slash_command)]
pub async fn source(ctx: Context<'_>) -> Result<(), Error> {
    poise::say_reply(ctx, r"https://github.com/kangalioo/rustbot".into()).await?;
    Ok(())
}

/// Show this menu
#[poise::command(track_edits, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"] command: Option<String>,
) -> Result<(), Error> {
    let bottom_text = "Type ?help command for more info on a command.
You can edit your message to the bot and the bot will edit its response.";
    poise::defaults::help(ctx, command.as_deref(), bottom_text).await?;
    Ok(())
}

pub async fn is_owner(ctx: crate::PrefixContext<'_>) -> Result<bool, Error> {
    Ok(ctx.msg.author.id.0 == 472029906943868929)
}

/// Register slash commands in this guild or globally
///
/// Run with no arguments to register in guild, run with argument "global" to register globally.
#[poise::command(check = "is_owner", hide_in_help)]
pub async fn register(ctx: PrefixContext<'_>, #[flag] global: bool) -> Result<(), Error> {
    let guild_id = ctx.msg.guild_id.ok_or("Must be called in guild")?;
    let commands = &ctx.framework.options().slash_options.commands;
    poise::say_prefix_reply(ctx, format!("Registering {} commands...", commands.len())).await?;
    for cmd in commands {
        if global {
            cmd.create_global(&ctx.discord.http).await?;
        } else {
            cmd.create_in_guild(&ctx.discord.http, guild_id).await?;
        }
    }
    poise::say_prefix_reply(ctx, "Done!".to_owned()).await?;
    Ok(())
}
