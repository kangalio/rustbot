use crate::{Context, Error};
use serenity::model::prelude::*;
use std::collections::HashMap;

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

/// Look up a guild member by a string, case-insensitively.
///
/// The lookup strategy is as follows (in order):
/// 1. Lookup by ID.
/// 2. Lookup by mention.
/// 3. Lookup by name#discrim
/// 4. Lookup by name
/// 5. Lookup by nickname
fn parse_member<'a>(members: &'a HashMap<UserId, Member>, string: &str) -> Option<&'a Member> {
    let lookup_by_id = || members.get(&UserId(string.parse().ok()?));

    let lookup_by_mention = || {
        members.get(&UserId(
            string
                .strip_prefix("<@!")
                .or_else(|| string.strip_prefix("<@"))?
                .strip_suffix(">")?
                .parse()
                .ok()?,
        ))
    };

    let lookup_by_name_and_discrim = || {
        let pound_sign = string.find('#')?;
        let name = &string[..pound_sign];
        let discrim = string[(pound_sign + 1)..].parse::<u16>().ok()?;
        members.values().find(|member| {
            member.user.discriminator == discrim && member.user.name.eq_ignore_ascii_case(name)
        })
    };

    let lookup_by_name = || members.values().find(|member| member.user.name == string);

    let lookup_by_nickname = || {
        members.values().find(|member| match &member.nick {
            Some(nick) => nick.eq_ignore_ascii_case(string),
            None => false,
        })
    };

    lookup_by_id()
        .or_else(lookup_by_mention)
        .or_else(lookup_by_name_and_discrim)
        .or_else(lookup_by_name)
        .or_else(lookup_by_nickname)
}

/// Bans another person
///
/// ?ban <member> [reason]
///
/// Bans another person
#[poise::command(on_error = "crate::react_cross")]
pub fn ban(ctx: Context<'_>, banned_user: String, reason: Option<String>) -> Result<(), Error> {
    let guild = ctx
        .msg
        .guild(ctx.discord)
        .await
        .ok_or("can't be used in DMs")?;
    let banned_user = parse_member(&guild.members, &banned_user)
        .ok_or("member not found")?
        .user
        .clone();

    poise::say_reply(
        ctx,
        format!(
            "{}#{} banned user {}#{}{}  {}",
            ctx.msg.author.name,
            ctx.msg.author.discriminator,
            banned_user.name,
            banned_user.discriminator,
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
pub async fn rustify(ctx: Context<'_>, users: Vec<String>) -> Result<(), Error> {
    let guild = ctx
        .msg
        .guild(&ctx.discord)
        .await
        .ok_or("can't be used in DMs")?;

    for user in users {
        parse_member(&guild.members, &user)
            .ok_or("member not found")?
            .clone()
            .add_role(&ctx.discord, ctx.data.rustacean_role)
            .await?;
    }

    crate::react_custom_emoji(ctx, "rustOk", '👌').await
}
