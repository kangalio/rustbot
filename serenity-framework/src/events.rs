use crate::Error;
use serenity::{model::prelude::*, prelude::*};

pub struct Events<U> {
    pub cmds: crate::Commands<U>,
}

impl<U: Send + Sync> EventHandler for Events<U> {
    fn ready(&self, ctx: Context, ready: Ready) {
        log::info!("{} connected to discord", ready.user.name);
        {
            let mut data = ctx.data.write();
            data.insert::<crate::CommandHistory>(indexmap::IndexMap::new());
            data.insert::<crate::BotUserIdKey>(ready.user.id);
        }

        std::thread::spawn(move || -> Result<(), Error> {
            loop {
                crate::clear_command_history(&ctx)?;
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        });
    }

    fn message(&self, ctx: Context, message: Message) {
        self.cmds.execute(&ctx, &message);
    }

    fn message_update(
        &self,
        ctx: Context,
        _: Option<Message>,
        _: Option<Message>,
        ev: MessageUpdateEvent,
    ) {
        if let Err(e) = crate::replay_message(ctx, ev, &self.cmds) {
            log::error!("{}", e);
        }
    }

    fn message_delete(&self, ctx: Context, channel_id: ChannelId, message_id: MessageId) {
        let mut data = ctx.data.write();
        let history = data.get_mut::<crate::CommandHistory>().unwrap();
        if let Some(response_id) = history.remove(&message_id) {
            log::info!("deleting message: {:?}", response_id);
            let _ = channel_id.delete_message(&ctx, response_id);
        }
    }
}
