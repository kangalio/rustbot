use crate::{Context, Error};

/// Evaluates Go code
#[poise::command(discard_spare_arguments, slash_command)]
pub async fn go(ctx: Context<'_>) -> Result<(), Error> {
    poise::say_reply(ctx, "No".into()).await?;
    Ok(())
}

/// Information about this bot
///
/// Information about this bot, for example a link to the GitHub repo.
#[poise::command(discard_spare_arguments, slash_command)]
pub async fn about(ctx: Context<'_>) -> Result<(), Error> {
    // TODO: add more info...? At some point I had ideas about what to put here but I forgot
    poise::say_reply(ctx, r"GitHub: https://github.com/kangalioo/rustbot".into()).await?;
    Ok(())
}

/// Show this menu
#[poise::command(track_edits, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"] command: Option<String>,
) -> Result<(), Error> {
    let reply = if let Some(command) = command {
        if let Some(command) = ctx
            .framework()
            .options()
            .prefix_options
            .commands
            .iter()
            .find(|cmd| cmd.name == command)
        {
            command
                .options
                .multiline_help
                .map_or("No help available".into(), |f| f())
        } else {
            format!("No such command `{}`", command)
        }
    } else {
        let mut menu = "```\nCommands:\n".to_owned();
        for command in &ctx.framework().options().prefix_options.commands {
            menu += &format!(
                "\t?{:<12}{}\n",
                command.name,
                command.options.inline_help.unwrap_or("")
            );
        }
        menu += "\nType ?help command for more info on a command.";
        menu += "\nYou can edit your message to the bot and the bot will edit its response.";
        menu += "\n```";

        menu
    };

    poise::say_reply(ctx, reply).await?;

    Ok(())
}
