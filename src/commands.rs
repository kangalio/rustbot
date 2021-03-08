use crate::Error;
use reqwest::blocking::Client as HttpClient;
use serenity::{model::prelude::*, prelude::*};
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

pub enum CommandHandler {
    Help,
    Custom {
        action: Box<dyn Fn(&Args<'_>) -> Result<(), Error> + Send + Sync>,
        /// Multiline description of the command to display for the command-specific help command
        help: Box<dyn Fn(&Args<'_>) -> Result<(), Error> + Send + Sync>,
    },
}

pub struct Command {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub broadcast_typing: bool,
    /// Should be a short sentence to display inline in the help menu
    pub inline_help: &'static str,
    pub handler: CommandHandler,
}

pub struct Args<'a> {
    pub http: &'a HttpClient,
    pub cx: &'a Context,
    pub msg: &'a Message,
    pub params: HashMap<&'a str, &'a str>,
    pub body: &'a str,
}

pub struct Commands {
    client: HttpClient,
    commands: Vec<Command>,
}

impl Commands {
    pub fn new() -> Self {
        Self {
            client: HttpClient::new(),
            commands: vec![Command {
                name: "help",
                aliases: &[],
                broadcast_typing: false,
                inline_help: "Show this menu",
                handler: CommandHandler::Help,
            }],
        }
    }

    pub fn add(
        &mut self,
        command: &'static str,
        handler: impl Fn(&Args) -> Result<(), Error> + Send + Sync + 'static,
        inline_help: &'static str,
        long_help: impl Fn(&Args) -> Result<(), Error> + Send + Sync + 'static,
    ) -> &mut Command {
        self.commands.push(Command {
            name: command,
            aliases: &[],
            broadcast_typing: false,
            inline_help,
            handler: CommandHandler::Custom {
                action: Box::new(handler),
                help: Box::new(long_help),
            },
        });
        self.commands.last_mut().unwrap()
    }

    pub fn help_menu(&self, args: &Args) -> Result<(), Error> {
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
                Some(cmd) => match &cmd.handler {
                    CommandHandler::Help => crate::api::send_reply(args, "Are you beyond help?"),
                    CommandHandler::Custom { help, .. } => (help)(args),
                },
                None => crate::api::send_reply(args, &format!("No such command `{}`", args.body)),
            }
        }
    }

    fn find_command<'a>(&'a self, command_name: &str) -> Option<&'a Command> {
        self.commands.iter().find(|cmd| {
            let command_matches = cmd.name.eq_ignore_ascii_case(command_name);
            let alias_matches = cmd
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(command_name));
            command_matches || alias_matches
        })
    }

    // Takes the (optional) param list (optionally) followed by a body, and parses it.
    // Returns the key=value HashMap, and the body, in a tuple.
    fn parse_argument_list<'a>(msg: &'a str) -> (HashMap<&'a str, &'a str>, &'a str) {
        // Some commands like to pre-declare possible params (or "flags" if you wish)
        // Specifying arguments comes in two flavours:
        // 1) ?command param1=value-without-spaces param2=other-value body goes here
        // 2) ?command
        //      param1= put whatever space separated thing you want here
        //      param2= here too
        //      actual body follows on a new line
        // We should handle both cases.

        let mut params = HashMap::new();

        // `parsing_stopped` is the last index into `msg` that we've successfully parsed before encountering an error
        let mut parsing_stopped: Option<usize> = None;
        'args_loop: for (block_no, block) in msg.splitn(2, '\n').enumerate() {
            let block = block.trim();
            let token_iter: Box<dyn Iterator<Item = &str>> = if block_no == 0 {
                Box::new(block.split_whitespace())
            } else {
                Box::new(block.lines())
            };
            for token in token_iter {
                let mut splitn_2 = token.splitn(2, '=');
                match (splitn_2.next(), splitn_2.next()) {
                    (Some(param_name), Some(param_val)) => {
                        // Check that the param key is sensible, otherwise any equal sign in arg body
                        // (think ?eval) will be parsed as a parameter
                        if !param_name.chars().all(|c| c.is_alphanumeric()) {
                            parsing_stopped = Some(token.as_ptr() as usize - msg.as_ptr() as usize);
                            break 'args_loop;
                        }
                        params.insert(param_name, param_val);
                    }
                    _ => {
                        parsing_stopped = Some(token.as_ptr() as usize - msg.as_ptr() as usize);
                        break 'args_loop;
                    }
                };
            }
        }
        // When the key=value list stops - the body starts
        let body = match parsing_stopped {
            None => "",
            Some(index) => &msg[index..],
        };

        return (params, body);
    }

    pub fn execute(&self, cx: &Context, serenity_msg: &Message) {
        // find the first matching prefix and strip it
        let msg = match PREFIXES
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

        let (params, body) = Self::parse_argument_list(msg);
        let args = Args {
            body,
            params,
            cx: &cx,
            msg: &serenity_msg,
            http: &self.client,
        };

        if command.broadcast_typing {
            if let Err(e) = serenity_msg.channel_id.broadcast_typing(&cx.http) {
                warn!("Can't broadcast typing: {}", e);
            }
        }

        let command_execution_result = match &command.handler {
            CommandHandler::Help => self.help_menu(&args),
            CommandHandler::Custom { action, .. } => (action)(&args),
        };
        if let Err(e) = command_execution_result {
            error!("Error when executing command {}: {}", command.name, e);
            if let Err(e) = crate::api::send_reply(&args, &e.to_string()) {
                error!("{}", e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Commands;
    #[test]
    fn check_arg_parsing() {
        struct TestCase {
            input: &'static str,
            expected: (&'static [(&'static str, &'static str)], &'static str),
        }
        let test_cases = vec![
            TestCase {
                input: "",
                expected: (&[], ""),
            },
            TestCase {
                input: "hello",
                expected: (&[], "hello"),
            },
            TestCase {
                input: "hello=world",
                expected: (&[("hello", "world")], ""),
            },
            TestCase {
                input: "hello=world it's me",
                expected: (&[("hello", "world")], "it's me"),
            },
            TestCase {
                input: "it's=me",
                expected: (&[], "it's=me"),
            },
            TestCase{
                input: "\nflags=-C opt-level=3\nrustc=1.3.0\n```code```",
                expected: (&[("flags", "-C opt-level=3"), ("rustc", "1.3.0")], "```code```"),
            }
        ];
        for (i, tc) in test_cases.into_iter().enumerate() {
            let got = Commands::parse_argument_list(tc.input);
            let expected = (tc.expected.0.iter().cloned().collect(), tc.expected.1);
            assert_eq!(got, expected, "test case #{}", i);
        }
    }
}
