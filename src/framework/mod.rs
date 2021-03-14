//! This modules handles command dispatch and updating bot responses when the user edits their
//! message

mod api;
pub use api::*;

mod command_history;
pub use command_history::*;

mod commands;
pub use commands::*;

use crate::Error;
use serenity::{model::prelude::*, prelude::*};

pub struct BotUserId;

impl TypeMapKey for BotUserId {
    type Value = UserId;
}

pub struct Events {
    pub cmds: Commands,
}

impl EventHandler for Events {
    fn ready(&self, cx: Context, ready: Ready) {
        info!("{} connected to discord", ready.user.name);
        {
            let mut data = cx.data.write();
            data.insert::<command_history::CommandHistory>(Vec::new());
            data.insert::<BotUserId>(ready.user.id);
        }

        std::thread::spawn(move || -> Result<(), Error> {
            loop {
                command_history::clear_command_history(&cx)?;
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        });
    }

    fn message(&self, cx: Context, message: Message) {
        self.cmds.execute(&cx, &message);
    }

    fn message_update(
        &self,
        cx: Context,
        _: Option<Message>,
        _: Option<Message>,
        ev: MessageUpdateEvent,
    ) {
        command_history::apply_message_update(cx, ev, &self.cmds);
    }

    fn message_delete(&self, cx: Context, channel_id: ChannelId, message_id: MessageId) {
        let mut data = cx.data.write();
        let history = data.get_mut::<command_history::CommandHistory>().unwrap();
        if let Some(history_entry_index) = history
            .iter()
            .position(|entry| entry.user_message.id == message_id)
        {
            history.remove(history_entry_index);
            let _ = channel_id.delete_message(&cx, history[history_entry_index].response.id);
        }
    }
}
