use crate::{Args, Error};
use serenity::model::prelude::*;
use std::collections::HashMap;

pub async fn cleanup(args: &Args<'_>, mod_role_id: RoleId) -> Result<(), Error> {
    let num_messages = if args.body.is_empty() {
        5
    } else {
        args.body.parse::<usize>()?
    };

    info!("Cleaning up {} messages", num_messages);

    let is_mod = match &args.msg.member {
        Some(member) => member.roles.contains(&mod_role_id),
        None => true, // in DMs, treat the user as an "effective" mod
    };
    let data = args.cx.data.read().await;
    let bot_id = *data.get::<crate::BotUserId>().unwrap();

    // try_for_each would be much nicer for this but alas async support in Rust is still in baby
    // stages (and yet everyone and their dog switches to async, sigh)
    for message_to_be_deleted in args
        .msg
        .channel_id
        .messages(&args.cx.http, |m| m.limit(100))
        .await?
        .iter()
        .filter(|msg| {
            if msg.author.id != bot_id {
                return false;
            }
            if is_mod {
                return true;
            }
            if (msg.timestamp - args.msg.timestamp).num_hours() >= 24 {
                return false;
            }
            true
        })
        .take(num_messages)
    {
        message_to_be_deleted.delete(&args.cx.http).await?;
    }

    crate::react_custom_emoji(args, "rustOk", '👌').await
}

pub async fn cleanup_help(args: &Args<'_>) -> Result<(), Error> {
    crate::api::send_reply(
        args,
        "?cleanup [limit]

Deletes the bot's messages for cleanup.
You can specify how many messages to look for. Only messages from the last 24 hours can be deleted,
except for mods",
    )
    .await
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

pub async fn joke_ban(args: &Args<'_>) -> Result<(), Error> {
    let guild_id = match args.msg.guild_id {
        Some(x) => x,
        None => return crate::api::send_reply(args, "🤨").await,
    };

    let bannee = if let Some(first_arg) = args.body.split_whitespace().next() {
        guild_id
            .to_guild_cached(&args.cx.cache)
            .await
            .and_then(|guild| parse_member(&guild.members, first_arg).map(|m| m.user.clone()))
    } else {
        None
    };

    match bannee {
        Some(bannee) => {
            crate::api::send_reply(
                args,
                &format!(
                    "{}#{} banned user {}#{} {}",
                    args.msg.author.name,
                    args.msg.author.discriminator,
                    bannee.name,
                    bannee.discriminator,
                    crate::custom_emoji_code(args, "ferrisBanne", '🔨').await,
                ),
            )
            .await?
        }
        None => {
            args.msg
                .react(&args.cx.http, ReactionType::from('❌'))
                .await?;
        }
    }

    Ok(())
}

pub async fn joke_ban_help(args: &Args<'_>) -> Result<(), Error> {
    crate::api::send_reply(
        args,
        "?ban <member>

Bans another person",
    )
    .await
}
