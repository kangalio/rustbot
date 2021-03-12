#[macro_use]
extern crate log;

mod code_execution;
mod crates;
mod framework;
mod moderation;

use framework::{send_reply, Args, Commands};
use serenity::{model::prelude::*, prelude::*};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

fn app() -> Result<(), Error> {
    let discord_token = std::env::var("DISCORD_TOKEN").map_err(|_| "Missing DISCORD_TOKEN")?;
    let mod_role_id = std::env::var("MOD_ROLE_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or("Missing MOD_ROLE_ID")?;

    let mut cmds = Commands::new();

    cmds.add(
        "crate",
        crates::search,
        "Lookup crates on crates.io",
        crates::help,
    )
    .broadcast_typing = true;

    let mut docs_cmd = cmds.add(
        "docs",
        crates::doc_search,
        "Lookup documentation",
        crates::doc_help,
    );
    docs_cmd.broadcast_typing = true;
    docs_cmd.aliases = &["doc"];

    cmds.add(
        "play",
        code_execution::play,
        "Compile and run Rust code in a playground",
        |args| code_execution::play_and_eval_help(args, "play"),
    )
    .broadcast_typing = true;

    cmds.add(
        "eval",
        code_execution::eval,
        "Evaluate a single Rust expression",
        |args| code_execution::play_and_eval_help(args, "eval"),
    )
    .broadcast_typing = true;

    cmds.add(
        "miri",
        code_execution::miri,
        "Run code and detect undefined behavior using Miri",
        code_execution::miri_help,
    )
    .broadcast_typing = true;

    cmds.add(
        "expand",
        code_execution::expand_macros,
        "Expand macros to their raw desugared form",
        code_execution::expand_macros_help,
    )
    .broadcast_typing = true;

    cmds.add(
        "clippy",
        code_execution::clippy,
        "Catch common mistakes using the Clippy linter",
        code_execution::clippy_help,
    )
    .broadcast_typing = true;

    cmds.add(
        "fmt",
        code_execution::fmt,
        "Format code using rustfmt",
        code_execution::fmt_help,
    )
    .broadcast_typing = true;

    cmds.add(
        "microbench",
        code_execution::micro_bench,
        "Benchmark small snippets of code",
        code_execution::micro_bench_help,
    )
    .broadcast_typing = true;

    cmds.add(
        "go",
        |args| framework::send_reply(args, "No"),
        "Evaluates Go code",
        |args| framework::send_reply(args, "Evaluates Go code"),
    );

    cmds.add(
        "godbolt",
        code_execution::godbolt,
        "View assembly using Godbolt",
        code_execution::godbolt_help,
    )
    .broadcast_typing = true;

    cmds.add(
        "cleanup",
        move |args| moderation::cleanup(args, RoleId(mod_role_id)),
        "Deletes the bot's messages for cleanup",
        moderation::cleanup_help,
    );

    cmds.add(
        "ban",
        moderation::joke_ban,
        "Bans another person",
        moderation::joke_ban_help,
    )
    .aliases = &["banne"];

    cmds.add(
        "source",
        |args| framework::send_reply(args, "https://github.com/kangalioo/rustbot"),
        "Links to the bot GitHub repo",
        |args| framework::send_reply(args, "?source\n\nLinks to the bot GitHub repo"),
    );

    Client::new_with_extras(&discord_token, |e| {
        e.event_handler(framework::Events { cmds })
    })?
    .start()?;
    Ok(())
}

pub fn find_custom_emoji(args: &Args, emoji_name: &str) -> Option<Emoji> {
    args.msg.guild(&args.cx.cache).and_then(|guild| {
        guild
            .read()
            .emojis
            .values()
            .find(|emoji| emoji.name.eq_ignore_ascii_case(emoji_name))
            .cloned()
    })
}

pub fn custom_emoji_code(args: &Args, emoji_name: &str, fallback: char) -> String {
    match find_custom_emoji(args, emoji_name) {
        Some(emoji) => emoji.to_string(),
        None => fallback.to_string(),
    }
}

// React with a custom emoji from the guild, or fallback to a default Unicode emoji
pub fn react_custom_emoji(args: &Args, emoji_name: &str, fallback: char) -> Result<(), Error> {
    let reaction = find_custom_emoji(args, emoji_name)
        .map(ReactionType::from)
        .unwrap_or_else(|| ReactionType::from(fallback));

    args.msg.react(&args.cx.http, reaction)?;
    Ok(())
}

pub fn main() {
    env_logger::init();

    if let Err(e) = app() {
        error!("{}", e);
        std::process::exit(1);
    }
}
