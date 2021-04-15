use crate::{Context, Error};

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
            if (msg.timestamp - ctx.created_at()).num_hours() >= 24 {
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
#[poise::command(on_error = "crate::acknowledge_fail", slash_command)]
pub async fn ban(
    ctx: Context<'_>,
    #[description = "Banned user"] banned_user: serenity::Member,
    #[description = "Ban reason"] reason: Option<String>,
) -> Result<(), Error> {
    poise::say_reply(
        ctx,
        format!(
            "{}#{} banned user {}#{}{}  {}",
            ctx.author().name,
            ctx.author().discriminator,
            banned_user.user.name,
            banned_user.user.discriminator,
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

/// Adds the Rustacean role to a member
#[poise::command(on_error = "crate::acknowledge_fail", slash_command)]
pub async fn rustify(
    ctx: Context<'_>,
    // TODO: make this work with a list of users again
    // #[description = "List of users to rustify"] users: Vec<serenity::Member>,
    #[description = "User to rustify"] mut user: serenity::Member,
) -> Result<(), Error> {
    // for mut user in users {
    user.add_role(&ctx.discord(), ctx.data().rustacean_role)
        .await?;
    // }
    crate::acknowledge_success(ctx, "rustOk", '👌').await
}
