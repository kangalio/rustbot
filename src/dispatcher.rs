use serenity::{model::prelude::*, prelude::*, utils::parse_username, Client};
use crate::commands::Commands;

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

const WELCOME_BILLBOARD: &'static str = "By joining this community, you agree to adhere to the CoC.  Click the :white_check_mark: to indicate you agree, otherwise you can leave this Discord.  ";

pub(crate) struct EventDispatcher;

impl RawEventHandler for EventDispatcher {
    fn raw_event(&self, cx: Context, event: Event) {
        match event {
            Event::GuildCreate(ref ev) => {
                &ev.guild
                    .channels
                    .iter()
                    .filter(|(channel_id, _)| {
                        channel_id.name(&cx).unwrap_or_else(|| String::new()) == "welcome"
                    })
                    .for_each(|(channel_id, _)| {
                        let message = channel_id.say(&cx, WELCOME_BILLBOARD);
                        crate::MessageStore::save(&cx, "welcome".into(), message.unwrap());
                    });
            }
            Event::ReactionAdd(ref ev) => {
                if &ev.reaction.emoji == &ReactionType::from("✅") {
                    let channel = ev.reaction.channel(&cx).unwrap();
                    let user_id = ev.reaction.user_id;
                    let guild = channel.guild().unwrap();
                    let role_id = guild
                        .read()
                        .guild(&cx)
                        .unwrap()
                        .read()
                        .roles
                        .values()
                        .filter(|value| value.name == "talk")
                        .collect::<Vec<&Role>>()
                        .pop()
                        .map(|role| role.id)
                        .unwrap();

                    let guild_clone = guild.read().guild(&cx).unwrap().clone();
                    let mut member = guild_clone.read().member(&cx, &user_id).unwrap().clone();
                    member.add_role(&cx, role_id).unwrap();
                }
            }
            _ => (),
        }
    }
}
