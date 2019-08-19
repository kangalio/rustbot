use crate::{
    cache::{MessageCache, RoleIdCache},
    commands::Commands,
};
use serenity::{model::prelude::*, prelude::*};

type Result = crate::commands::Result<()>;

pub(crate) struct MessageDispatcher {
    cmds: Commands,
}

/// # Dispatcher
///
/// This is the event handler for all messages.   
impl MessageDispatcher {
    pub fn new(cmds: Commands) -> Self {
        Self { cmds }
    }
}

impl EventHandler for MessageDispatcher {
    fn message(&self, cx: Context, msg: Message) {
        self.cmds.execute(cx, msg);
    }

    fn ready(&self, _: Context, ready: Ready) {
        println!("{} connected", ready.user.name);
    }
}

pub(crate) struct EventDispatcher;

impl RawEventHandler for EventDispatcher {
    fn raw_event(&self, cx: Context, event: Event) {
        match event {
            Event::ReactionAdd(ref ev) => {
                if let Err(e) = assign_talk_role(&cx, ev) {
                    println!("{}", e);
                }
            }
            _ => (),
        }
    }
}

fn assign_talk_role(cx: &Context, ev: &ReactionAddEvent) -> Result {
    let data = cx.data.read();
    let reaction = &ev.reaction;

    if reaction.emoji == ReactionType::from("✅") {
        let channel = reaction.channel(cx)?;
        let channel_id = ChannelId::from(&channel);

        let message_store = data
            .get::<MessageCache>()
            .ok_or("Unable to access MessageCache")?;

        let role_store = data
            .get::<RoleIdCache>()
            .ok_or("Unable to access RoleIdCache")?;

        let (cached_message, cached_channel_id) = message_store
            .get("welcome".into())
            .ok_or("Unable to read from MessageCache")?;

        let message = reaction.message(cx)?;

        if message.id == cached_message.id && channel_id == *cached_channel_id {
            if let Some(talk_role) = role_store.get("talk".into()) {
                let user_id = reaction.user_id;

                let guild = channel
                    .guild()
                    .ok_or("Unable to retrieve guild from channel")?;

                let mut member = guild
                    .read()
                    .guild(&cx)
                    .ok_or("Unable to access guild")?
                    .read()
                    .member(cx, &user_id)?;

                member.add_role(&cx, talk_role)?;

                // Requires ManageMessage permission
                ev.reaction.delete(cx)?;
            }
        }
    }
    Ok(())
}
