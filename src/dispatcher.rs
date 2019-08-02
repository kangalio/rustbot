use crate::commands::Commands;
use serenity::{model::prelude::*, prelude::*, utils::parse_username, Client};
use std::collections::HashMap;

pub(crate) struct MessageStore;

impl TypeMapKey for MessageStore {
    type Value = HashMap<String, (Message, ChannelId)>;
}

impl MessageStore {
    pub(crate) fn init(client: &mut Client) {
        let mut data = client.data.write();
        data.insert::<Self>(HashMap::new());
    }

    pub(crate) fn save(cx: &Context, name: String, msg: (Message, ChannelId)) {
        let mut data = cx.data.write();
        let store = data
            .get_mut::<Self>()
            .expect("Unable to access message store.  ");
        store.insert(name, msg);
    }
}

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
            Event::ReactionAdd(ref ev) => assign_talk_role(cx, ev),
            _ => (),
        }
    }
}

fn assign_talk_role(cx: Context, ev: &ReactionAddEvent) {
    let data = cx.data.read();
    let reaction = &ev.reaction;

    let store = data
        .get::<MessageStore>()
        .expect("RawEventHandler: Unable to access message store.  ");

    let message = reaction
        .message(&cx)
        .expect("RawEventHandler: Unable to access message");

    let channel = reaction
        .channel(&cx)
        .expect("RawEventHandler: Unable to access channel");

    let channel_id = ChannelId::from(&channel);

    let (cached_message, cached_channel_id) = store
        .get("welcome".into())
        .expect("RawEventHandler: Unable to read from message store");

    let user_id = reaction.user_id;

    let guild = channel
        .guild()
        .expect("RawEventHandler: Unable to access guild");

    if reaction.emoji == ReactionType::from("✅")
        && message.id == cached_message.id
        && channel_id == *cached_channel_id
    {
        let (role_id, mut member) = guild
            .read()
            .guild(&cx)
            .map(|lock| {
                let guild_handle = lock.read();
                let role_id = guild_handle
                    .roles
                    .values()
                    .filter(|value| value.name == "talk")
                    .collect::<Vec<&Role>>()
                    .pop()
                    .map(|role| role.id)
                    .expect("RawEventHandler: Unable to access role");

                let member = guild_handle
                    .member(&cx, &user_id)
                    .expect("RawEventHandler: Unable to access member");
                (role_id, member)
            })
            .expect("RawEventHandler: role_id unable to acquire read lock on guild");

        member
            .add_role(&cx, role_id)
            .expect("RawEventHandler: Unable to add role");
    }
}
