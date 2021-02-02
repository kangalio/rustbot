use crate::{
    commands::{Commands, PREFIXES},
    Error,
};
use indexmap::IndexMap;
use serenity::{model::prelude::*, prelude::*, utils::CustomMessage};
use std::time::Duration;

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

pub fn cleanup(args: &crate::Args) -> Result<(), crate::Error> {
    let num_messages = if args.body.is_empty() {
        5
    } else {
        args.body.parse::<usize>()?
    };

    info!("Cleaning up {} messages", num_messages);

    let is_mod = crate::api::is_mod(args)?;
    let data = args.cx.data.read();
    let command_history = data.get::<CommandHistory>().unwrap();
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
            if let Some((&trigger_message, _)) = command_history
                .iter()
                .find(|&(_, &response)| response == msg.id)
            {
                // Only delete if the command caller triggered this bot response
                trigger_message == msg.id
            } else {
                // Trigger message can't be found; this stray message can definitely be deleted
                true
            }
        })
        .take(num_messages)
        .map(|msg| msg.delete(&args.cx.http))
        .collect::<Result<(), _>>()?;

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
