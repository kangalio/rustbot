mod slowmode;
pub use slowmode::slowmode;

use crate::{serenity, Context, Error};

/// Deletes the bot's messages for cleanup
///
/// ?cleanup [limit]
///
/// By default, only the most recent bot message is deleted (limit = 1).
///
/// Deletes the bot's messages for cleanup.
/// You can specify how many messages to look for. Only the 20 most recent messages within the
/// channel from the last 24 hours can be deleted.
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
    let num_messages = num_messages.unwrap_or(1);

    let messages_to_delete = ctx
        .channel_id()
        .messages(ctx.discord(), serenity::GetMessages::new().limit(20))
        .await?
        .into_iter()
        .filter(|msg| {
            if msg.author.id != ctx.data().bot_user_id {
                return false;
            }
            if (*ctx.created_at() - *msg.timestamp).num_hours() >= 24 {
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
/// ?ban <member>
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
    _reason: Option<String>,
) -> Result<(), Error> {
    ctx.say(format!(
        "Banned user {}  {}",
        banned_user.user.tag(),
        crate::custom_emoji_code(ctx, "ferrisBanne", '🔨').await
    ))
    .await?;
    Ok(())
}

async fn rustify_inner(ctx: Context<'_>, users: &[serenity::Member]) -> Result<(), Error> {
    if let Some(member) = ctx.author_member().await {
        if !member.roles.contains(&ctx.data().rustacean_role) {
            return Err("Only Rustaceans can use this command".into());
        }
    }
    if users.is_empty() {
        // This error text won't be seen (replaced with a cross emoji reaction)
        return Err("Please specify a user to rustify".into());
    }

    for user in users {
        ctx.discord()
            .http
            .add_member_role(
                user.guild_id,
                user.user.id,
                ctx.data().rustacean_role,
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
// rustify is the canonical one with all the attributes set correctly

/// Adds the Rustacean role to members
#[poise::command(
    prefix_command,
    on_error = "crate::acknowledge_fail",
    rename = "rustify",
    category = "Moderation",
    ephemeral
)]
pub async fn rustify(ctx: Context<'_>, users: Vec<serenity::Member>) -> Result<(), Error> {
    rustify_inner(ctx, &users).await
}

/// Adds the Rustacean role to a member
#[poise::command(slash_command, context_menu_command = "Rustify")]
pub async fn application_rustify(
    ctx: Context<'_>,
    #[description = "User to rustify"] user: serenity::User,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Must use this command in a guild")?;
    let member = guild_id.member(ctx.discord(), user.id).await?;
    rustify_inner(ctx, &[member]).await
}

async fn latest_message_link(ctx: Context<'_>) -> String {
    let message = ctx
        .channel_id()
        .messages(ctx.discord(), serenity::GetMessages::new().limit(1))
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

    let report_name = format!("Report {}", ctx.id() % 1000);

    // let msg = reports_channel.say(ctx.discord(), &report_name).await?;
    let mut report_thread = reports_channel
        // .create_public_thread(ctx.discord(), msg, serenity::CreateThread::new(report_name))
        .create_private_thread(ctx.discord(), serenity::CreateThread::new(report_name))
        .await?;
    // Prevent non-mods from unarchiving the thread and accidentally exposing themselves in audit log.
    report_thread
        .edit_thread(ctx.discord(), serenity::EditThread::new().locked(true))
        .await?;

    let thread_message_content = format!(
        "Hey <@&{}>, <@{}> sent a report from channel {}: {}\n> {}",
        ctx.data().mod_role_id,
        ctx.author().id,
        naughty_channel.name,
        latest_message_link(ctx).await,
        reason
    );
    report_thread
        .send_message(
            ctx.discord(),
            serenity::CreateMessage::new()
                .content(thread_message_content)
                .allowed_mentions(
                    serenity::CreateAllowedMentions::new()
                        .users(&[ctx.author().id])
                        .roles(&[ctx.data().mod_role_id]),
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
    let guild_id = ctx.guild_id().ok_or("Guild not in cache")?;
    let member = guild_id.member(ctx.discord(), ctx.author().id).await?;
    let permissions_in_target_channel = guild_id
        .to_partial_guild(ctx.discord())
        .await?
        .user_permissions_in(&target_channel, &member)?;
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
    if let Context::Prefix(ctx) = ctx {
        if let Some(referenced_message) = &ctx.msg.referenced_message {
            comefrom_message += "\n> ";
            comefrom_message += &referenced_message.content;
        }
    }

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
        .send_message(
            ctx.discord(),
            serenity::CreateMessage::new()
                .content(comefrom_message)
                .allowed_mentions(serenity::CreateAllowedMentions::new().users(users_to_ping)),
        )
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
