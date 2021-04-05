use crate::{Context, Error};

/// Evaluates Go code
#[poise::command]
pub fn go(ctx: Context<'_>, #[rest] _: &str) -> Result<(), Error> {
    poise::say_reply(ctx, "No".into()).await?;
    Ok(())
}

/// ?source
///
/// Links to the bot GitHub repo
#[poise::command]
pub fn source(ctx: Context<'_>, #[rest] _: &str) -> Result<(), Error> {
    poise::say_reply(ctx, "https://github.com/kangalioo/rustbot".into()).await?;
    Ok(())
}

#[poise::command(track_edits)]
pub fn help(ctx: Context<'_>, query: Option<String>) -> Result<(), Error> {
    let reply = if let Some(query) = query {
        if let Some(cmd) = ctx
            .framework
            .options()
            .commands
            .iter()
            .find(|cmd| cmd.name == query)
        {
            cmd.options
                .explanation
                .map_or("No help available".into(), |f| f())
        } else {
            format!("No such command `{}`", query)
        }
    } else {
        let mut menu = "```\nCommands:\n".to_owned();
        for command in &ctx.framework.options().commands {
            menu += &format!(
                "\t?{:<12}{}\n",
                command.name,
                command.options.description.unwrap_or("")
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
