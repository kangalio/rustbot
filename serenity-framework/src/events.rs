use crate::Error;
use serenity::{model::prelude::*, prelude::*};

pub struct Events {
    pub cmds: crate::Commands,
}

impl EventHandler for Events {
    fn ready(&self, cx: Context, ready: Ready) {
        log::info!("{} connected to discord", ready.user.name);
        {
            let mut data = cx.data.write();
            data.insert::<crate::CommandHistory>(indexmap::IndexMap::new());
            data.insert::<crate::BotUserIdKey>(ready.user.id);
        }

        std::thread::spawn(move || -> Result<(), Error> {
            loop {
                crate::clear_command_history(&cx)?;
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
        if let Err(e) = crate::replay_message(cx, ev, &self.cmds) {
            log::error!("{}", e);
        }
    }

    fn message_delete(&self, cx: Context, channel_id: ChannelId, message_id: MessageId) {
        let mut data = cx.data.write();
        let history = data.get_mut::<crate::CommandHistory>().unwrap();
        if let Some(response_id) = history.remove(&message_id) {
            log::info!("deleting message: {:?}", response_id);
            let _ = channel_id.delete_message(&cx, response_id);
        }
    }
}
