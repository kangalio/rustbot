use crate::Error;
use reqwest::blocking::Client as HttpClient;
use serenity::{model::prelude::*, prelude::*};
use std::collections::HashMap;

pub enum CommandHandler<U> {
    Help,
    Custom {
        action: fn(&Args<'_, U>) -> Result<(), Error>,
    },
}

pub struct Command<U> {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub broadcast_typing: bool,
    /// Should be a short sentence to display inline in the help menu
    pub inline_help: &'static str,
    pub multiline_help: String,
    pub handler: CommandHandler<U>,
}

pub struct Args<'a, U> {
    pub http: &'a HttpClient,
    pub ctx: &'a Context,
    pub msg: &'a Message,
    pub params: HashMap<&'a str, &'a str>,
    pub body: &'a str,
    pub user_data: &'a U,
}

impl<U> Args<'_, U> {
    pub fn bot_user_id(&self) -> UserId {
        *self.ctx.data.read().get::<crate::BotUserIdKey>().unwrap()
    }
}

pub struct Commands<U> {
    client: HttpClient,
    prefixes: &'static [&'static str],
    commands: Vec<Command<U>>,
    user_data: U,
}

impl<U> Commands<U> {
    pub fn new_with_help(prefixes: &'static [&'static str], user_data: U) -> Self {
        Self {
            client: HttpClient::new(),
            prefixes,
            commands: vec![Command {
                name: "help",
                aliases: &[],
                broadcast_typing: false,
                inline_help: "Show this menu",
                multiline_help: "Show a help menu with descriptions of all available commands"
                    .to_owned(),
                handler: CommandHandler::Help,
            }],
            user_data,
        }
    }

    pub fn add(
        &mut self,
        command: &'static str,
        handler: fn(&Args<U>) -> Result<(), Error>,
        inline_help: &'static str,
        multiline_help: String,
    ) -> &mut Command<U> {
        self.commands.push(Command {
            name: command,
            aliases: &[],
            broadcast_typing: false,
            inline_help,
            multiline_help,
            handler: CommandHandler::Custom { action: handler },
        });
        self.commands.last_mut().unwrap()
    }

    pub fn help_menu(&self, args: &Args<U>) -> Result<(), Error> {
        if args.body.is_empty() {
            let mut menu = "```\nCommands:\n".to_owned();
            for command in &self.commands {
                menu += &format!("\t?{:<12}{}\n", command.name, command.inline_help);
            }
            menu += "\nType ?help command for more info on a command.";
            menu += "\nYou can edit your message to the bot and the bot will edit its response.";
            menu += "\n```";

            crate::api::send_reply(args, &menu)
        } else {
            match self.find_command(&args.body) {
                Some(cmd) => crate::api::send_reply(args, &cmd.multiline_help),
                None => crate::api::send_reply(args, &format!("No such command `{}`", args.body)),
            }
        }
    }

    fn find_command<'a>(&'a self, command_name: &str) -> Option<&'a Command<U>> {
        self.commands.iter().find(|cmd| {
            let command_matches = cmd.name.eq_ignore_ascii_case(command_name);
            let alias_matches = cmd
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(command_name));
            command_matches || alias_matches
        })
    }

    pub fn execute(&self, ctx: &Context, serenity_msg: &Message) {
        // find the first matching prefix and strip it
        let msg = match self
            .prefixes
            .iter()
            .find_map(|prefix| serenity_msg.content.strip_prefix(prefix))
        {
            Some(x) => x,
            None => return,
        };

        // Find the command that matches this message
        let (command_name, msg) =
            msg.split_at(msg.find(char::is_whitespace).unwrap_or_else(|| msg.len()));
        let msg = msg.trim();
        let command = match self.find_command(command_name) {
            Some(x) => x,
            None => return,
        };

        let mut params = HashMap::new();
        let mut body = "";
        for token in msg.split_whitespace() {
            let mut splitn_2 = token.splitn(2, '=');
            if let (Some(param_name), Some(param_val)) = (splitn_2.next(), splitn_2.next()) {
                // Check that the param key is sensible, otherwise any equal sign in arg body
                // (think ?eval) will be parsed as a parameter
                if param_name.chars().all(|c| c.is_alphanumeric()) {
                    params.insert(param_name, param_val);
                    continue;
                }
            }
            // If this whitespace-separated token is not a "key=value" pair, this must
            // be the beginning of the command body. So, let's find out where we are within
            // the msg string and set the body accordingly
            let body_start = token.as_ptr() as usize - msg.as_ptr() as usize;
            body = &msg[body_start..];
            break;
        }

        let args = Args {
            body,
            params,
            ctx: &ctx,
            msg: &serenity_msg,
            http: &self.client,
            user_data: &self.user_data,
        };

        if command.broadcast_typing {
            if let Err(e) = serenity_msg.channel_id.broadcast_typing(&ctx.http) {
                log::warn!("Can't broadcast typing: {}", e);
            }
        }

        let command_execution_result = match &command.handler {
            CommandHandler::Help => self.help_menu(&args),
            CommandHandler::Custom { action, .. } => (action)(&args),
        };
        if let Err(e) = command_execution_result {
            log::error!("Error when executing command {}: {}", command.name, e);
            if let Err(e) = crate::api::send_reply(&args, &e.to_string()) {
                log::error!("{}", e)
            }
        }
    }
}
