use crate::{
    commands::{Commands, PREFIX},
    Error, SendSyncError, HOUR,
};
use indexmap::IndexMap;
use serenity::{model::prelude::*, prelude::*, utils::CustomMessage};
use std::time::Duration;

const MESSAGE_AGE_MAX: Duration = Duration::from_secs(HOUR);

pub(crate) struct CommandHistory;

impl TypeMapKey for CommandHistory {
    type Value = IndexMap<MessageId, MessageId>;
}

pub(crate) fn replay_message(
    cx: Context,
    ev: MessageUpdateEvent,
    cmds: &Commands,
) -> Result<(), Error> {
    let age = ev.timestamp.and_then(|create| {
        ev.edited_timestamp
            .and_then(|edit| edit.signed_duration_since(create).to_std().ok())
    });

    if age.is_some() && age.unwrap() < MESSAGE_AGE_MAX {
        let mut msg = CustomMessage::new();
        msg.id(ev.id)
            .channel_id(ev.channel_id)
            .content(ev.content.unwrap_or_else(|| String::new()));

        let msg = msg.build();

        if msg.content.starts_with(PREFIX) {
            info!(
                "sending edited message - {:?} {:?}",
                msg.content, msg.author
            );
            cmds.execute(cx, &msg);
        }
    }

    Ok(())
}

pub(crate) fn clear_command_history(cx: &Context) -> Result<(), SendSyncError> {
    let mut data = cx.data.write();
    let history = data.get_mut::<CommandHistory>().unwrap();

    // always keep the last command in history
    if history.len() > 0 {
        info!("Clearing command history");
        history.drain(..history.len() - 1);
    }
    Ok(())
}
