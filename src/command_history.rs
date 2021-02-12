use crate::{
    commands::{Commands, PREFIXES},
    Error,
};
use indexmap::IndexMap;
use serenity::{model::prelude::*, prelude::*, utils::CustomMessage};
use std::{collections::HashMap, time::Duration};

const MAX_EDIT_TRACKING_MESSAGE_AGE: Duration = Duration::from_secs(3600);

pub struct CommandHistory;

impl TypeMapKey for CommandHistory {
    type Value = IndexMap<MessageId, MessageId>;
}

pub fn replay_message(cx: Context, ev: MessageUpdateEvent, cmds: &Commands) -> Result<(), Error> {
    let age = ev.timestamp.and_then(|create| {
        ev.edited_timestamp
            .and_then(|edit| edit.signed_duration_since(create).to_std().ok())
    });

    if age.is_some() && age.unwrap() < MAX_EDIT_TRACKING_MESSAGE_AGE {
        let mut msg = CustomMessage::new();
        msg.id(ev.id)
            .channel_id(ev.channel_id)
            .content(ev.content.unwrap_or_else(String::new));

        let msg = msg.build();

        if PREFIXES.iter().any(|p| msg.content.starts_with(p)) {
            info!(
                "sending edited message - {:?} {:?}",
                msg.content, msg.author
            );
            cmds.execute(&cx, &msg);
        }
    }

    Ok(())
}

pub fn clear_command_history(cx: &Context) -> Result<(), Error> {
    let mut data = cx.data.write();
    let history = data.get_mut::<CommandHistory>().unwrap();

    // always keep the last command in history
    if !history.is_empty() {
        info!("Clearing command history");
        history.drain(..history.len() - 1);
    }
    Ok(())
}

pub fn cleanup(args: &crate::Args, mod_role_id: RoleId) -> Result<(), crate::Error> {
    let num_messages = if args.body.is_empty() {
        5
    } else {
        args.body.parse::<usize>()?
    };

    info!("Cleaning up {} messages", num_messages);

    let is_mod = match &args.msg.member {
        Some(member) => member.roles.contains(&mod_role_id),
        None => true, // in DMs, the user is "effectively" a mod
    };
    let data = args.cx.data.read();
    let bot_id = *data.get::<crate::BotUserId>().unwrap();

    args.msg
        .channel_id
        .messages(&args.cx.http, |m| m.limit(100))?
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
        .map(|msg| msg.delete(&args.cx.http))
        .collect::<Result<(), _>>()?;

    // Find the :rustOk: emoji on this server, or fallback to the normal Ok emoji
    let rust_ok = if let Some(guild_id) = args.msg.guild_id {
        fn find_rust_ok(emojis: &HashMap<EmojiId, Emoji>) -> Option<&Emoji> {
            emojis
                .values()
                .find(|emoji| emoji.name.eq_ignore_ascii_case("rustOk"))
        }

        match guild_id.to_guild_cached(args.cx) {
            Some(cached_guild) => find_rust_ok(&cached_guild.read().emojis).cloned(),
            None => find_rust_ok(&guild_id.to_partial_guild(args.cx)?.emojis).cloned(),
        }
    } else {
        None
    };
    let reaction = match rust_ok {
        Some(rust_ok) => ReactionType::Custom {
            animated: rust_ok.animated,
            name: Some(rust_ok.name.to_owned()),
            id: rust_ok.id,
        },
        None => ReactionType::Unicode("👌".to_owned()),
    };

    // React with the emoji we found
    args.msg.react(args.cx, reaction)?;

    Ok(())
}

pub fn cleanup_help(args: &crate::Args) -> Result<(), crate::Error> {
    crate::api::send_reply(
        args,
        "?cleanup [limit]

Deletes the bot's messages for cleanup.
You can specify how many messages to look for. Only messages from the last 24 hours are deleted,
and only messages triggered by the user who calls this command. Mods ",
    )
}
