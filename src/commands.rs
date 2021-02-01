use crate::Error;
use indexmap::IndexMap;
use reqwest::blocking::Client as HttpClient;
use serenity::{model::channel::Message, prelude::Context};
use std::collections::HashMap;

pub const PREFIXES: &[&str] = &[
    "?",
    "🦀 ",
    "🦀",
    "<:ferris:358652670585733120> ",
    "<:ferris:358652670585733120>",
    "hey ferris can you please ",
    "hey ferris, can you please ",
    "hey fewwis can you please ",
    "hey fewwis, can you please ",
    "hey ferris can you ",
    "hey ferris, can you ",
    "hey fewwis can you ",
    "hey fewwis, can you ",
];
pub type GuardFn = fn(&Args) -> Result<bool, Error>;

struct Command {
    name: String,
    guard: GuardFn,
    handler: Box<dyn Fn(Args<'_>) -> Result<(), Error> + Send + Sync>,
}

#[derive(Clone)]
pub struct Args<'a> {
    pub http: &'a HttpClient,
    pub cx: &'a Context,
    pub msg: &'a Message,
    pub params: &'a HashMap<&'a str, &'a str>,
    pub body: &'a str,
}

pub struct Commands {
    client: HttpClient,
    menu: Option<IndexMap<&'static str, (&'static str, GuardFn)>>,
    new_commands: Vec<Command>,
}

impl Commands {
    pub fn new() -> Self {
        Self {
            client: HttpClient::new(),
            menu: Some(IndexMap::new()),
            new_commands: Vec::new(),
        }
    }

    pub fn add(
        &mut self,
        command: &'static str,
        handler: impl Fn(Args) -> Result<(), Error> + Send + Sync + 'static,
    ) {
        self.add_protected(command, handler, |_| Ok(true));
    }

    pub fn add_protected(
        &mut self,
        command: &'static str,
        handler: impl Fn(Args) -> Result<(), Error> + Send + Sync + 'static,
        guard: GuardFn,
    ) {
        self.new_commands.push(Command {
            name: command.to_owned(),
            guard,
            handler: Box::new(handler),
        });
    }

    pub fn help(
        &mut self,
        cmd: &'static str,
        desc: &'static str,
        handler: impl Fn(Args) -> Result<(), Error> + Send + Sync + 'static,
    ) {
        self.help_protected(cmd, desc, handler, |_| Ok(true));
    }

    pub fn help_protected(
        &mut self,
        cmd: &'static str,
        desc: &'static str,
        handler: impl Fn(Args) -> Result<(), Error> + Send + Sync + 'static,
        guard: GuardFn,
    ) {
        info!("Adding command ?help {}", &cmd);

        self.menu.as_mut().map(|menu| {
            menu.insert(cmd, (desc, guard));
            menu
        });

        self.new_commands.push(Command {
            name: format!("help {}", cmd),
            guard,
            handler: Box::new(handler),
        });
    }

    pub fn take_menu(&mut self) -> Option<IndexMap<&'static str, (&'static str, GuardFn)>> {
        self.menu.take()
    }

    pub fn execute(&self, cx: &Context, serenity_msg: &Message) {
        // find the first matching prefix and strip it
        let msg = match PREFIXES
            .iter()
            .filter_map(|prefix| serenity_msg.content.strip_prefix(prefix))
            .next()
        {
            Some(x) => x,
            None => return,
        };

        for command in &self.new_commands {
            // Extract "body" from something like "command_name body"
            let msg = match msg.strip_prefix(&command.name) {
                Some(msg) => msg.trim(),
                None => continue,
            };

            let mut params = HashMap::new();
            let mut body = "";
            for token in msg.split_whitespace() {
                let mut splitn_2 = token.splitn(2, '=');
                if let (Some(param_name), Some(param_val)) = (splitn_2.next(), splitn_2.next()) {
                    params.insert(param_name, param_val);
                } else {
                    // If this whitespace-separated token is not a "key=value" pair, this must
                    // be the beginning of the command body. So, let's find out where we are within
                    // the msg string and set the body accordingly
                    let body_start = token.as_ptr() as usize - msg.as_ptr() as usize;
                    body = &msg[body_start..];
                    break;
                }
            }

            let args = Args {
                body,
                params: &params,
                cx: &cx,
                msg: serenity_msg,
                http: &self.client,
            };

            match (command.guard)(&args) {
                Ok(true) => {
                    if let Err(e) = (command.handler)(args.clone()) {
                        error!("Error when executing command {}: {}", command.name, e);
                        if let Err(e) =
                            crate::api::send_reply(&args, &format!("Encountered error ({})", e))
                        {
                            error!("{}", e)
                        }
                    }
                }
                Ok(false) => {} // user doesn't have permission for command
                Err(e) => {
                    error!(
                        "Error when checking command permissions for {}: {}",
                        command.name, e
                    );
                    if let Err(e) =
                        crate::api::send_reply(&args, &format!("Encountered error ({})", e))
                    {
                        error!("{}", e)
                    }
                }
            }
        }
    }
}
