use crate::commands::Commands;
use serenity::{model::prelude::*, prelude::*, utils::parse_username, Client};

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
                let data = cx.data.read();
                let store = data
                    .get::<crate::MessageStore>()
                    .expect("Unable to access message store.  ");

                if &ev.reaction.emoji == &ReactionType::from("✅")
                    && store
                        .get("welcome".into())
                        .expect("RawEventHandler: Unable to read from message store")
                        .id
                        == *&ev
                            .reaction
                            .message(&cx)
                            .expect("RawEventHandler: Unable to access message")
                            .id
                {
                    let channel = ev
                        .reaction
                        .channel(&cx)
                        .expect("RawEventHandler: Unable to access channel");
                    let user_id = ev.reaction.user_id;
                    let guild = channel
                        .guild()
                        .expect("RawEventHandler: Unable to access guild");
                    let role_id = guild
                        .read()
                        .guild(&cx)
                        .expect("RawEventHandler: Unable to acquire read lock on guild")
                        .read()
                        .roles
                        .values()
                        .filter(|value| value.name == "talk")
                        .collect::<Vec<&Role>>()
                        .pop()
                        .map(|role| role.id)
                        .expect("RawEventHandler: Unable to access role");

                    let guild_clone = guild
                        .read()
                        .guild(&cx)
                        .expect("RawEventHandler: Unable to acquire read lock clone on guild")
                        .clone();
                    let mut member = guild_clone
                        .read()
                        .member(&cx, &user_id)
                        .expect("RawEventHandler: Unable to access member")
                        .clone();

                    member
                        .add_role(&cx, role_id)
                        .expect("RawEventHandler: Unable to add role");
                }
            }
            _ => (),
        }
    }
}
