use crate::{
    cache::{self, MessageCache, RoleIdCache},
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
            Event::GuildCreate(ref ev) => {
                if let Err(e) = init(ev) {
                    println!("{}", e);
                }
            }
            Event::ReactionAdd(ref ev) => {
                if let Err(e) = assign_talk_role(&cx, ev) {
                    println!("{}", e);
                }
            }
            _ => (),
        }
    }
}

fn init(ev: &GuildCreateEvent) -> Result {
    let guild = &ev.guild;

    let mod_role = guild
        .role_by_name("mod".into())
        .ok_or("Unable to fetch mod role")?
        .id;

    let talk_role = guild
        .role_by_name("talk".into())
        .ok_or("Unable to fetch talk role")?
        .id;

    cache::save_or_update_role("mod", mod_role)?;
    cache::save_or_update_role("talk", talk_role)?;

    Ok(())
}

fn assign_talk_role(cx: &Context, ev: &ReactionAddEvent) -> Result {
    let reaction = &ev.reaction;

    if reaction.emoji == ReactionType::from("✅") {
        let channel = reaction.channel(cx)?;
        let channel_id = ChannelId::from(&channel);
        let msg = MessageCache::get_by_name("welcome")?;
        let role = RoleIdCache::get_by_name("talk")?;

        if let Some((_, _, cached_message_id, cached_channel_id)) = msg {
            let message = reaction.message(cx)?;

            if message.id.0.to_string() == cached_message_id
                && channel_id.0.to_string() == *cached_channel_id
            {
                if let Some((_, role_id, _)) = role {
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

                    use std::str::FromStr;
                    member.add_role(&cx, RoleId::from(u64::from_str(&role_id)?))?;

                    // Requires ManageMessage permission
                    ev.reaction.delete(cx)?;
                }
            }
        }
    }
    Ok(())
}
