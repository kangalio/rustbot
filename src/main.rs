mod commands;
mod state_machine;

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

fn assign_talk_role<'m>(args: Args<'m>) -> Result {
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

    Ok(())
}

fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("env var not set");
    let mut cmds = Commands::new();

    cmds.add("?talk", assign_talk_role);

    let mut client = Client::new(&token, Dispatcher::new(cmds)).unwrap();

    if let Err(e) = client.start() {
        println!("{}", e);
    }
}
