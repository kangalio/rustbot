use crate::commands::Commands;
use serenity::{model::prelude::*, prelude::*};
use crate::db::DB;
use crate::schema::{messages, roles, users};
use diesel::prelude::*;

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
    let reaction = &ev.reaction;

    let channel = reaction.channel(cx)?;
    let channel_id = ChannelId::from(&channel);
    let message = reaction.message(cx)?;

    let conn = DB.get()?;

    let (msg, talk_role, me) = conn
        .build_transaction()
        .read_only()
        .run::<_, Box<dyn std::error::Error>, _>(|| {
            let msg: Option<_> = messages::table
                .filter(messages::name.eq("welcome"))
                .first::<(i32, String, String, String)>(&conn)
                .optional()?;

            let role: Option<_> = roles::table
                .filter(roles::name.eq("talk"))
                .first::<(i32, String, String)>(&conn)
                .optional()?;

            let me: Option<_> = users::table
                .filter(users::name.eq("me"))
                .first::<(i32, String, String)>(&conn)
                .optional()?;

            Ok((msg, role, me))
        })?;

    if let Some((_, _, cached_message_id, cached_channel_id)) = msg {
        if message.id.0.to_string() == cached_message_id
            && channel_id.0.to_string() == *cached_channel_id
        {
            if reaction.emoji == ReactionType::from("✅") {
                if let Some((_, role_id, _)) = talk_role {
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
                    if let Some((_, _, user_id)) = me {
                        if ev.reaction.user_id.0.to_string() != user_id {
                            ev.reaction.delete(cx)?;
                        }
                    }
                }
            } else {
                ev.reaction.delete(cx)?;
            }
        }
    }
    Ok(())
}
