#[macro_use]
extern crate diesel;

mod api;
mod commands;
mod db;
mod schema;
mod state_machine;
mod tags;

use commands::{Args, Commands, Result};
use serenity::{model::prelude::*, prelude::*, utils::parse_username, Client};
use std::str::FromStr;

struct Dispatcher {
    cmds: Commands,
}

/// # Dispatcher
///
/// This is the event handler for all messages.   
impl Dispatcher {
    fn new(cmds: Commands) -> Self {
        Self { cmds }
    }
}

impl EventHandler for Dispatcher {
    fn message(&self, cx: Context, msg: Message) {
        self.cmds.execute(cx, msg);
    }

    fn ready(&self, _: Context, ready: Ready) {
        println!("{} connected", ready.user.name);
    }
}

fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("env var not set");
    let mut cmds = Commands::new();

    // Talk Role
    cmds.add("?talk", assign_talk_role);

    // Tags
    cmds.add("?tag {key}", tags::get);
    cmds.add("?tag delete {key}", tags::delete);
    cmds.add("?tag create {key} [value]", tags::post);
    cmds.add("?tags", tags::get_all);

    // Slow mode.
    // 0 seconds disables slowmode
    cmds.add("?slowmode {channel} {seconds}", slow_mode);

    // Kick
    cmds.add("?kick {user}", kick);

    // Ban
    cmds.add("?ban {user}", ban);

    let mut client = Client::new(&token, Dispatcher::new(cmds)).unwrap();

    if let Err(e) = client.start() {
        println!("{}", e);
    }
}

/// Assign the talk role to the user that requested it.  
fn assign_talk_role<'m>(args: Args<'m>) -> Result {
    if api::channel_name_is(&args, "welcome") {
        if let Some(ref guild) = args.msg.guild(&args.cx) {
            let role_id = guild
                .read()
                .role_by_name("talk")
                .ok_or("unable to retrieve role")?
                .id;

            args.msg
                .member(&args.cx)
                .ok_or("unable to retrieve member")?
                .add_role(&args.cx, role_id)?;
        }
    }

    Ok(())
}

/// Set slow mode for a channel.  
///
/// A `seconds` value of 0 will disable slowmode
fn slow_mode<'m>(args: Args<'m>) -> Result {
    let seconds = &args
        .params
        .get("seconds")
        .ok_or("unable to retrieve seconds param")?
        .parse::<u64>()?;

    let channel_name = &args
        .params
        .get("channel")
        .ok_or("unable to retrieve channel param")?;

    ChannelId::from_str(channel_name)?.edit(&args.cx, |c| c.slow_mode_rate(*seconds))?;
    Ok(())
}

/// Kick a user from the guild.  
///
/// Requires the kick members permission
fn kick<'m>(args: Args<'m>) -> Result {
    if let Some(guild) = args.msg.guild(&args.cx) {
        let role_id = guild
            .read()
            .role_by_name("mod")
            .ok_or("unable to retrieve role")?
            .id;

        if args
            .msg
            .member
            .ok_or("unable to retrieve member")?
            .roles
            .contains(&role_id)
        {
            let user_id = parse_username(
                &args
                    .params
                    .get("user")
                    .ok_or("unable to retrieve user param")?,
            )
            .ok_or("unable to retrieve user id")?;
            guild.read().kick(&args.cx, UserId::from(user_id))?
        }
    }
    Ok(())
}

/// Ban an user from the guild.  
///
/// Requires the ban members permission
fn ban<'m>(args: Args<'m>) -> Result {
    if let Some(guild) = args.msg.guild(&args.cx) {
        let role_id = guild
            .read()
            .role_by_name("mod")
            .ok_or("unable to retrieve role")?
            .id;

        if args
            .msg
            .member
            .ok_or("unable to retrieve member")?
            .roles
            .contains(&role_id)
        {
            let user_id = parse_username(
                &args
                    .params
                    .get("user")
                    .ok_or("unable to retrieve user param")?,
            )
            .ok_or("unable to retrieve user id")?;
            guild.read().ban(&args.cx, UserId::from(user_id), &"all")?
        }
    }
    Ok(())
}
