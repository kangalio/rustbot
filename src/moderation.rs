use crate::{serenity, Context, Error, PrefixContext};

/// Deletes the bot's messages for cleanup
///
/// ?cleanup [limit]
///
/// Deletes the bot's messages for cleanup.
/// You can specify how many messages to look for. Only messages from the last 24 hours can be deleted.
#[poise::command(on_error = "crate::acknowledge_fail", slash_command)]
pub async fn cleanup(
    ctx: Context<'_>,
    #[description = "Number of messages to delete"] num_messages: Option<usize>,
) -> Result<(), Error> {
    let num_messages = num_messages.unwrap_or(5);

    let messages_to_delete = ctx
        .channel_id()
        .messages(ctx.discord(), |m| m.limit(100))
        .await?
        .into_iter()
        .filter(|msg| {
            if msg.author.id != ctx.data().bot_user_id {
                return false;
            }
            if (ctx.created_at() - msg.timestamp).num_hours() >= 24 {
                return false;
            }
            true
        })
        .take(num_messages);

    ctx.channel_id()
        .delete_messages(ctx.discord(), messages_to_delete)
        .await?;

    crate::acknowledge_success(ctx, "rustOk", '👌').await
}

/// Bans another person
///
/// ?ban <member> [reason]
///
/// Bans another person
#[poise::command(
    on_error = "crate::acknowledge_fail",
    aliases("banne"),
    slash_command,
    track_edits
)]
pub async fn ban(
    ctx: Context<'_>,
    #[description = "Banned user"] banned_user: serenity::Member,
    #[description = "Ban reason"]
    #[rest]
    reason: Option<String>,
) -> Result<(), Error> {
    poise::say_reply(
        ctx,
        format!(
            "Banned user {}{}  {}",
            banned_user.user.tag(),
            match reason {
                Some(reason) => format!(" {}", reason.trim()),
                None => String::new(),
            },
            crate::custom_emoji_code(ctx, "ferrisBanne", '🔨').await
        ),
    )
    .await?;
    Ok(())
}

async fn rustify_inner(ctx: Context<'_>, users: &[serenity::Member]) -> Result<(), Error> {
    if users.is_empty() {
        // This error message won't be seen
        return Err("Please specify a user to rustify".into());
    }

    for user in users {
        ctx.discord()
            .http
            .add_member_role(
                user.guild_id.0,
                user.user.id.0,
                ctx.data().rustacean_role.0,
                Some(&format!(
                    "You have been rusted by {}! owo",
                    ctx.author().name
                )),
            )
            .await?;
    }
    crate::acknowledge_success(ctx, "rustOk", '👌').await
}

// We need separate implementations for the rustify command, because the slash command only supports
// a single argument while the normal (prefix) version supports variadic arguments

/// Adds the Rustacean role to members
#[poise::command(on_error = "crate::acknowledge_prefix_fail", rename = "rustify")]
pub async fn prefix_rustify(
    ctx: PrefixContext<'_>,
    users: Vec<serenity::Member>,
) -> Result<(), Error> {
    rustify_inner(Context::Prefix(ctx), &users).await
}

/// Adds the Rustacean role to a member
#[poise::command(
    on_error = "crate::acknowledge_fail",
    slash_command,
    ephemeral,
    rename = "rustify"
)]
pub async fn slash_rustify(
    ctx: Context<'_>,
    #[description = "User to rustify"] user: serenity::Member,
) -> Result<(), Error> {
    rustify_inner(ctx, &[user]).await
}

/// Discreetly report a user for breaking the rules
#[poise::command(slash_command, ephemeral, hide_in_help)]
pub async fn report(
    ctx: Context<'_>,
    #[description = "What did the user do wrong?"] reason: String,
) -> Result<(), Error> {
    let slash_ctx = match ctx {
        poise::Context::Slash(ctx) => ctx,
        _ => return Ok(()),
    };

    let reports_channel = ctx
        .data()
        .reports_channel
        .ok_or("No reports channel was configured")?;

    let naughty_channel = ctx
        .channel_id()
        .to_channel(ctx.discord())
        .await?
        .guild()
        .ok_or("This command can only be used in a guild")?;

    let naughty_message = naughty_channel
        .messages(ctx.discord(), |f| f.limit(1))
        .await?
        .into_iter()
        .next()
        .ok_or("Couldn't retrieve latest message in channel")?;

    reports_channel
        .say(
            ctx.discord(),
            format!(
                "{} sent a report from channel {}: {}\n> {}",
                ctx.author().name,
                naughty_channel.name,
                naughty_message.link_ensured(ctx.discord()).await,
                reason
            ),
        )
        .await?;

    poise::say_slash_reply(
        slash_ctx,
        "Successfully sent report. Thanks for helping to make this community a better place!"
            .into(),
    )
    .await?;

    Ok(())
}

/// Move a discussion to another channel
///
/// Move a discussion to a specified channel, optionally pinging a list of users in the new channel.
#[poise::command(rename = "move", aliases("migrate"))]
pub async fn move_(
    ctx: PrefixContext<'_>,
    channel: serenity::GuildChannel,
    users_to_ping: Vec<serenity::Member>,
) -> Result<(), Error> {
    use serenity::Mentionable as _;

    if Some(channel.guild_id) != ctx.msg.guild_id {
        return Err("Can't move discussion across servers".into());
    }

    let mut comefrom_message = format!(
        "**Discussion moved here from {}**\n{}",
        ctx.msg.channel_id.mention(),
        ctx.msg.link_ensured(ctx.discord).await
    );

    let mut users_to_ping = users_to_ping.into_iter();
    if let Some(user_to_ping) = users_to_ping.next() {
        comefrom_message += &format!("\n{}", user_to_ping.mention());
        for user_to_ping in users_to_ping {
            comefrom_message += &format!(", {}", user_to_ping.mention());
        }
    }

    let comefrom_message = channel.say(ctx.discord, comefrom_message).await?;

    poise::say_prefix_reply(
        ctx,
        format!(
            "**{} suggested to move this discussion to {}**\n{}",
            &ctx.msg.author.tag(),
            channel.mention(),
            comefrom_message.link_ensured(ctx.discord).await
        ),
    )
    .await?;

    Ok(())
}
