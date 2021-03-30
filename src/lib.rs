mod code_execution;
mod crates;
mod moderation;

use serenity::model::prelude::*;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

// Wraps a command function and reacts with a red cross emoji on error
fn react_cross(error: Error, ctx: poise::CommandErrorContext<Data, Error>) {
    println!("Reacting with red cross because of error: {}", error);
    if let Err(e) = ctx.ctx.msg.react(ctx.ctx.discord, ReactionType::from('❌')) {
        println!("Failed to react with red cross: {}", e);
    }
}

pub struct Data {
    bot_user_id: UserId,
    mod_role_id: RoleId,
    rustacean_role: RoleId,
    http: reqwest::blocking::Client,
}

fn app() -> Result<(), Error> {
    let discord_token = std::env::var("DISCORD_TOKEN").map_err(|_| "Missing DISCORD_TOKEN")?;
    let mod_role_id = std::env::var("MOD_ROLE_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or("Missing/invalid MOD_ROLE_ID")?;
    let rustacean_role = std::env::var("RUSTACEAN_ROLE_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or("Missing/invalid RUSTACEAN_ROLE_ID")?;

    let commands = vec![
        poise::Command {
            name: "crate",
            action: crates::search,
            options: poise::CommandOptions {
                broadcast_typing: Some(true),
                description: Some("Lookup crates on crates.io"),
                explanation: Some(String::from(
                    "Search for a crate on crates.io
```
?crate crate_name
```",
                )),
                ..Default::default()
            },
        },
        poise::Command {
            name: "doc",
            action: crates::doc_search,
            options: poise::CommandOptions {
                broadcast_typing: Some(true),
                aliases: &["docs"],
                description: Some("Lookup documentation"),
                explanation: Some(String::from(
                    "Retrieve documentation for a given crate
```
?docs crate_name::module::item
```",
                )),
                ..Default::default()
            },
        },
        poise::Command {
            name: "play",
            action: code_execution::play,
            options: poise::CommandOptions {
                description: Some("Compile and run Rust code in a playground"),
                explanation: Some(code_execution::play_and_eval_help("play")),
                broadcast_typing: Some(true),
                ..Default::default()
            },
        },
        poise::Command {
            name: "eval",
            action: code_execution::eval,
            options: poise::CommandOptions {
                description: Some("Evaluate a single Rust expression"),
                explanation: Some(code_execution::play_and_eval_help("eval")),
                broadcast_typing: Some(true),
                ..Default::default()
            },
        },
        poise::Command {
            name: "miri",
            action: code_execution::miri,
            options: poise::CommandOptions {
                description: Some("Run code and detect undefined behavior using Miri"),
                explanation: Some(code_execution::miri_help()),
                broadcast_typing: Some(true),
                ..Default::default()
            },
        },
        poise::Command {
            name: "expand",
            action: code_execution::expand_macros,
            options: poise::CommandOptions {
                description: Some("Expand macros to their raw desugared form"),
                explanation: Some(code_execution::expand_macros_help()),
                broadcast_typing: Some(true),
                ..Default::default()
            },
        },
        poise::Command {
            name: "clippy",
            action: code_execution::clippy,
            options: poise::CommandOptions {
                description: Some("Catch common mistakes using the Clippy linter"),
                explanation: Some(code_execution::clippy_help()),
                broadcast_typing: Some(true),
                ..Default::default()
            },
        },
        poise::Command {
            name: "fmt",
            action: code_execution::fmt,
            options: poise::CommandOptions {
                description: Some("Format code using rustfmt"),
                explanation: Some(code_execution::fmt_help()),
                broadcast_typing: Some(true),
                ..Default::default()
            },
        },
        poise::Command {
            name: "microbench",
            action: code_execution::micro_bench,
            options: poise::CommandOptions {
                description: Some("Benchmark small snippets of code"),
                explanation: Some(code_execution::micro_bench_help()),
                broadcast_typing: Some(true),
                ..Default::default()
            },
        },
        poise::Command {
            name: "godbolt",
            action: code_execution::godbolt,
            options: poise::CommandOptions {
                broadcast_typing: Some(true),
                description: Some("View assembly using Godbolt"),
                explanation: Some(String::from(
                    "Compile Rust code using https://rust.godbolt.org. Full optimizations are applied unless overriden.
```?godbolt
``\u{200B}`
pub fn your_function() {
    // Code
}
``\u{200B}` ```
Optional arguments:
    \t`flags`: flags to pass to rustc invocation. Defaults to `-Copt-level=3 --edition=2018`
    \t`rustc`: compiler version to invoke. Defaults to `nightly`. Possible values: `nightly`, `beta` or full version like `1.45.2`
"
                )),
                ..Default::default()
            },
        },
        poise::Command {
            name: "go",
            action: |ctx, _args| {
                poise::say_reply(ctx, "No".into())?;
                Ok(())
            },
            options: poise::CommandOptions {
                description: Some("Evaluates Go code"),
                ..Default::default()
            }
        },
        poise::Command {
            name: "cleanup",
            action: moderation::cleanup,
            options: poise::CommandOptions {
                description: Some("Deletes the bot's messages for cleanup"),
                explanation: Some(String::from(
                    "?cleanup [limit]

Deletes the bot's messages for cleanup.
You can specify how many messages to look for. Only messages from the last 24 hours can be deleted,
except for mods"
                )),
                ..Default::default()
            }
        },
        poise::Command {
            name: "ban",
            action: moderation::joke_ban,
            options: poise::CommandOptions {
                aliases: &["banne"],
                on_error: Some(react_cross),
                description: Some("Bans another person"),
                explanation: Some(String::from(
                    "?ban <member> [reason]

Bans another person"
                )),
                ..Default::default()
            }
        },
        poise::Command {
            name: "rustify",
            action: moderation::rustify,
            options: poise::CommandOptions {
                aliases: &["wustify"],
                on_error: Some(react_cross),
                description: Some("Adds the Rustacean role to a member"),
                explanation: Some(String::from(
                    "\\?rustify <member>

Adds the Rustacean role to a member."
                )),
                ..Default::default()
            }
        },
        poise::Command {
            name: "source",
            action: |ctx, _args| {
                poise::say_reply(ctx, "https://github.com/kangalioo/rustbot".into())?;
                Ok(())
            },
            options: poise::CommandOptions {
                description: Some("Links to the bot GitHub repo"),
                explanation: Some(String::from("?source\n\nLinks to the bot GitHub repo")),
                ..Default::default()
            }
        },
        poise::Command {
            name: "help",
            action: |ctx, args| {
                let query = poise::parse_args!(args => (Option<String>))?;

                let reply = if let Some(query) = query {
                    if let Some(cmd) = ctx.framework.options().commands.iter().find(|cmd| cmd.name == query) {
                        cmd.options.explanation.as_deref().unwrap_or("No help available").to_owned()
                    } else {
                        format!("No such command `{}`", query)
                    }
                } else {
                    let mut menu = "```\nCommands:\n".to_owned();
                    for command in &ctx.framework.options().commands {
                        menu += &format!("\t?{:<12}{}\n", command.name, command.options.description.unwrap_or(""));
                    }
                    menu += "\nType ?help command for more info on a command.";
                    menu += "\nYou can edit your message to the bot and the bot will edit its response.";
                    menu += "\n```";

                    menu
                };

                poise::say_reply(ctx, reply)?;

                Ok(())
            },
            options: poise::CommandOptions {
                ..Default::default()
            }
        }
    ];

    let framework = poise::Framework::new(
        "?",
        move |_, bot| Data {
            bot_user_id: bot.user.id,
            mod_role_id,
            rustacean_role,
            http: reqwest::blocking::Client::new(),
        },
        poise::FrameworkOptions {
            commands,
            additional_prefixes: &[
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
            ],
            edit_tracker: Some(poise::EditTracker::for_timespan(
                std::time::Duration::from_secs(3600),
            )),
            on_error: |error, ctx| {
                if let poise::ErrorContext::Command(ctx) = ctx {
                    let reply = if let Some(poise::ArgumentParseError(error)) = error.downcast_ref()
                    {
                        if error.is::<poise::CodeBlockError>() {
                            "Missing code block. Please use the following markdown:
\\`code here\\`
or
\\`\\`\\`rust
code here
\\`\\`\\`"
                                .to_owned()
                        } else if let Some(explanation) = &ctx.command.options.explanation {
                            format!("**{}**\n{}", error, explanation)
                        } else {
                            error.to_string()
                        }
                    } else {
                        error.to_string()
                    };
                    if let Err(e) = poise::say_reply(ctx.ctx, reply) {
                        log::warn!("{}", e);
                    }
                }
            },
            track_edits_by_default: true,
            ..Default::default()
        },
    );

    framework.start(&discord_token)?;
    Ok(())
}

pub fn find_custom_emoji(ctx: Context<'_>, emoji_name: &str) -> Option<Emoji> {
    ctx.msg.guild(ctx.discord).and_then(|guild| {
        guild
            .read()
            .emojis
            .values()
            .find(|emoji| emoji.name.eq_ignore_ascii_case(emoji_name))
            .cloned()
    })
}

pub fn custom_emoji_code(ctx: Context<'_>, emoji_name: &str, fallback: char) -> String {
    match find_custom_emoji(ctx, emoji_name) {
        Some(emoji) => emoji.to_string(),
        None => fallback.to_string(),
    }
}

// React with a custom emoji from the guild, or fallback to a default Unicode emoji
pub fn react_custom_emoji(ctx: Context<'_>, emoji_name: &str, fallback: char) -> Result<(), Error> {
    let reaction = find_custom_emoji(ctx, emoji_name)
        .map(ReactionType::from)
        .unwrap_or_else(|| ReactionType::from(fallback));

    ctx.msg.react(ctx.discord, reaction)?;
    Ok(())
}

pub fn main() {
    env_logger::init();

    if let Err(e) = app() {
        log::error!("{}", e);
        std::process::exit(1);
    }
}
