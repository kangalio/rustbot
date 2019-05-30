#[macro_use]
extern crate diesel;

mod api;
mod commands;
mod state_machine;
mod tags;
mod db;
mod schema;

use commands::{Args, Commands, Result};
use serenity::{model::prelude::*, prelude::*, Client};

struct Dispatcher {
    cmds: Commands,
}

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
