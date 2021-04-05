use crate::{Context, Error};
use serenity::model::prelude::*;

/// Deletes the bot's messages for cleanup
///
/// ?cleanup [limit]
///
/// Deletes the bot's messages for cleanup.
/// You can specify how many messages to look for. Only messages from the last 24 hours can be deleted,
/// except for mods
#[poise::command(on_error = "crate::react_cross")]
pub async fn cleanup(ctx: Context<'_>, num_messages: Option<usize>) -> Result<(), Error> {
    let num_messages = num_messages.unwrap_or(5);

    println!("Cleaning up {} messages", num_messages);

    let is_mod = match &ctx.msg.member {
        Some(member) => member.roles.contains(&ctx.data.mod_role_id),
        None => true, // in DMs, treat the user as an "effective" mod
    };

    let messages_to_delete = ctx
        .msg
        .channel_id
        .messages(ctx.discord, |m| m.limit(100))
        .await?
        .into_iter()
        .filter(|msg| {
            if msg.author.id != ctx.data.bot_user_id {
                return false;
            }
            if is_mod {
                return true;
            }
            if (msg.timestamp - ctx.msg.timestamp).num_hours() >= 24 {
                return false;
            }
            true
        });

    ctx.msg
        .channel_id
        .delete_messages(ctx.discord, messages_to_delete)
        .await?;

    crate::react_custom_emoji(ctx, "rustOk", '👌').await
}

/// Bans another person
///
/// ?ban <member> [reason]
///
/// Bans another person
#[poise::command(on_error = "crate::react_cross")]
pub async fn ban(
    ctx: Context<'_>,
    banned_user: Member,
    reason: Option<String>,
) -> Result<(), Error> {
    poise::say_reply(
        ctx,
        format!(
            "{}#{} banned user {}#{}{}  {}",
            ctx.msg.author.name,
            ctx.msg.author.discriminator,
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

#[poise::command(on_error = "crate::react_cross")]
pub async fn rustify(ctx: Context<'_>, users: Vec<Member>) -> Result<(), Error> {
    for mut user in users {
        user.add_role(&ctx.discord, ctx.data.rustacean_role).await?;
    }
    crate::react_custom_emoji(ctx, "rustOk", '👌').await
}
