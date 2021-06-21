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
#[poise::command(on_error = "crate::acknowledge_fail", aliases("banne"), slash_command)]
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
            "Banned user {}#{:0>4}{}  {}",
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

async fn rustify_inner(ctx: Context<'_>, users: &[serenity::Member]) -> Result<(), Error> {
    for user in users {
        ctx.discord()
            .http
            .add_member_role(
                user.guild_id.0,
                user.user.id.0,
                ctx.data().rustacean_role.0,
                ctx.author()
                    .map(|author| format!("You have been rusted by {}! owo", author.name))
                    .as_deref(),
            )
            .await?;
    }
    crate::acknowledge_success(ctx, "rustOk", '👌').await
}

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
    rename = "rustify"
)]
pub async fn slash_rustify(
    ctx: Context<'_>,
    #[description = "User to rustify"] user: serenity::Member,
) -> Result<(), Error> {
    rustify_inner(ctx, &[user]).await
}
