use crate::{commands::Commands, Error};
use indexmap::IndexMap;
use serenity::{model::prelude::*, prelude::*, utils::CustomMessage};

pub struct CommandHistory;

impl TypeMapKey for CommandHistory {
    type Value = IndexMap<MessageId, MessageId>;
}

pub fn replay_message(cx: Context, ev: MessageUpdateEvent, cmds: &Commands) -> Result<(), Error> {
    if let (Some(created), Some(edited)) = (ev.timestamp, ev.edited_timestamp) {
        // Only track edits for recent messages
        if (edited - created).num_minutes() < 60 {
            let mut msg = CustomMessage::new();
            msg.id(ev.id)
                .channel_id(ev.channel_id)
                .content(ev.content.unwrap_or_else(String::new));
            cmds.execute(&cx, &msg.build());
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
