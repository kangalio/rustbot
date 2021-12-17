mod slowmode;
pub use slowmode::slowmode;

use crate::{serenity, Context, Error};

/// Deletes the bot's messages for cleanup
///
/// ?cleanup [limit]
///
/// Deletes the bot's messages for cleanup.
/// You can specify how many messages to look for. Only messages from the last 24 hours can be deleted.
#[poise::command(
    prefix_command,
    on_error = "crate::acknowledge_fail",
    slash_command,
    category = "Moderation"
)]
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
    prefix_command,
    on_error = "crate::acknowledge_fail",
    aliases("banne"),
    slash_command,
    track_edits,
    category = "Moderation"
)]
pub async fn ban(
    ctx: Context<'_>,
    #[description = "Banned user"] banned_user: serenity::Member,
    #[description = "Ban reason"]
    #[rest]
    reason: Option<String>,
) -> Result<(), Error> {
    ctx.say(format!(
        "Banned user {}{}  {}",
        banned_user.user.tag(),
        match reason {
            Some(reason) => format!(" {}", reason.trim()),
            None => String::new(),
        },
        crate::custom_emoji_code(ctx, "ferrisBanne", '🔨').await
    ))
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
#[poise::command(
    prefix_command,
    on_error = "crate::acknowledge_fail",
    rename = "rustify",
    category = "Moderation"
)]
pub async fn prefix_rustify(ctx: Context<'_>, users: Vec<serenity::Member>) -> Result<(), Error> {
    rustify_inner(ctx, &users).await
}

/// Adds the Rustacean role to a member
#[poise::command(
    prefix_command,
    slash_command,
    on_error = "crate::acknowledge_fail",
    ephemeral,
    rename = "rustify"
)]
pub async fn slash_rustify(
    ctx: Context<'_>,
    #[description = "User to rustify"] member: serenity::Member,
) -> Result<(), Error> {
    rustify_inner(ctx, &[member]).await
}

#[poise::command(prefix_command, context_menu_command = "Rustify", ephemeral)]
pub async fn context_menu_rustify(ctx: Context<'_>, user: serenity::User) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must use this command in a guild")?;
    let member = guild_id.member(ctx.discord(), user.id).await?;
    rustify_inner(ctx, &[member]).await
}

async fn latest_message_link(ctx: Context<'_>) -> String {
    let message = ctx
        .channel_id()
        .messages(ctx.discord(), |f| f.limit(1))
        .await
        .ok()
        .and_then(|messages| messages.into_iter().next());
    match message {
        Some(msg) => msg.link_ensured(ctx.discord()).await,
        None => "<couldn't retrieve latest message link>".into(),
    }
}

/// Discreetly reports a user for breaking the rules
///
/// Call this command in a channel when someone might be breaking the rules, for example by being \
/// very rude, or starting discussions about divisive topics like politics and religion. Nobody \
/// will see that you invoked this command.
///
/// Your report, along with a link to the \
/// channel and its most recent message, will show up in a dedicated reports channel for \
/// moderators, and it allows them to deal with it much faster than if you were to DM a \
/// potentially AFK moderator.
///
/// You can still always ping the Moderator role if you're comfortable doing so.
#[poise::command(slash_command, ephemeral, hide_in_help, category = "Moderation")]
pub async fn report(
    ctx: Context<'_>,
    #[description = "What did the user do wrong?"] reason: String,
) -> Result<(), Error> {
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

    reports_channel
        .say(
            ctx.discord(),
            format!(
                "{} sent a report from channel {}: {}\n> {}",
                ctx.author().name,
                naughty_channel.name,
                latest_message_link(ctx).await,
                reason
            ),
        )
        .await?;

    ctx.say("Successfully sent report. Thanks for helping to make this community a better place!")
        .await?;

    Ok(())
}

/// Move a discussion to another channel
///
/// Move a discussion to a specified channel, optionally pinging a list of users in the new channel.
#[poise::command(
    prefix_command,
    slash_command,
    rename = "move",
    aliases("migrate"),
    category = "Moderation"
)]
pub async fn move_(
    ctx: Context<'_>,
    #[description = "Where to move the discussion"] target_channel: serenity::GuildChannel,
    #[description = "Participants of the discussion who will be pinged in the new channel"]
    users_to_ping: Vec<serenity::Member>,
) -> Result<(), Error> {
    use serenity::Mentionable as _;

    if Some(target_channel.guild_id) != ctx.guild_id() {
        return Err("Can't move discussion across servers".into());
    }

    // DON'T use GuildChannel::permissions_for_user - it requires member to be cached
    let guild = ctx.guild().ok_or("Guild not in cache")?;
    let member = guild.member(ctx.discord(), ctx.author().id).await?;
    let permissions_in_target_channel = guild.user_permissions_in(&target_channel, &member)?;
    if !permissions_in_target_channel.send_messages() {
        return Err(format!(
            "You don't have permission to post in {}",
            target_channel.mention(),
        )
        .into());
    }

    let source_msg_link = match ctx {
        Context::Prefix(ctx) => ctx.msg.link_ensured(ctx.discord).await,
        _ => latest_message_link(ctx).await,
    };

    let mut comefrom_message = format!(
        "**Discussion moved here from {}**\n{}",
        ctx.channel_id().mention(),
        source_msg_link
    );

    {
        let mut users_to_ping = users_to_ping.iter();
        if let Some(user_to_ping) = users_to_ping.next() {
            comefrom_message += &format!("\n{}", user_to_ping.mention());
            for user_to_ping in users_to_ping {
                comefrom_message += &format!(", {}", user_to_ping.mention());
            }
        }
    }

    // let comefrom_message = target_channel.say(ctx.discord, comefrom_message).await?;
    let comefrom_message = target_channel
        .send_message(ctx.discord(), |f| {
            f.content(comefrom_message)
                .allowed_mentions(|f| f.users(users_to_ping))
        })
        .await?;

    ctx.say(format!(
        "**{} suggested to move this discussion to {}**\n{}",
        &ctx.author().tag(),
        target_channel.mention(),
        comefrom_message.link_ensured(ctx.discord()).await
    ))
    .await?;

    Ok(())
}
