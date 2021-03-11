use crate::{Context, Error};

use serenity::model::prelude::*;
use serenity_framework::prelude::*;

#[command]
/// Deletes the bot's messages for cleanup.
/// You can specify how many messages to look for. Only messages from the last 24 hours can be
/// deleted, except for mods.
pub async fn cleanup(
    ctx: FrameworkContext<crate::Data>,
    command_msg: &Message,
    limit: Option<usize>,
) -> Result<(), Error> {
    let num_messages_to_delete = limit.unwrap_or(5);

    info!("Cleaning up {} messages", num_messages_to_delete);

    let is_mod = match &command_msg.member {
        Some(member) => member.roles.contains(&ctx.data.mod_role_id),
        None => true, // in DMs, treat the user as an "effective" mod
    };

    // try_for_each would be much nicer for this but alas async support in Rust is still in
    // baby stages (and yet everyone and their dog switches to async, sigh)

    let mut num_deleted = 0;
    for msg in command_msg
        .channel_id
        .messages(&ctx.serenity_ctx.http, |m| m.limit(100))
        .await?
    {
        if msg.author.id == *ctx.data.bot_user_id.lock().await
            && !is_mod
            && (command_msg.timestamp - msg.timestamp).num_hours() >= 24
        {
            msg.delete(&ctx.serenity_ctx.http).await?;
            num_deleted += 1;
            if num_deleted == num_messages_to_delete {
                break;
            }
        }
    }

    crate::react_custom_emoji(&ctx, command_msg, "rustOk", '👌').await
}

#[command]
/// Bans another person
pub async fn ban(
    ctx: Context,
    msg: &Message,
    #[parse]
    #[rest]
    bannee: Member,
) -> Result<(), Error> {
    msg.channel_id
        .say(
            &ctx.serenity_ctx.http,
            &format!(
                "{}#{} banned user {}#{} {}",
                msg.author.name,
                msg.author.discriminator,
                bannee.user.name,
                bannee.user.discriminator,
                // crate::custom_emoji_code(args, "ferrisBanne", '🔨').await,
                "INSERT FERRIS BANNE HERE",
            ),
        )
        .await?;

    Ok(())
}
