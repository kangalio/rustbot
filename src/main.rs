mod commands;
mod state_machine;

use commands::{Args, Commands};
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

fn assign_talk_role<'m>(args: Args<'m>) {
    dbg!("assign_talk_role invoked");
}

fn ban<'m>(args: Args<'m>) {
    println!("{}", args.params.get("user").unwrap());
    dbg!("ban invoked");
}

fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("env var not set");
    let mut cmds = Commands::new();

    cmds.add("?talk", assign_talk_role);
    cmds.add("!ban {user} [reason]", ban);

    let mut client = Client::new(&token, Dispatcher::new(cmds)).unwrap();

    if let Err(e) = client.start() {
        println!("{}", e);
    }
}
