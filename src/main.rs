#[macro_use]
extern crate diesel;

#[macro_use]
extern crate diesel_migrations;

#[macro_use]
extern crate log;

mod api;
mod commands;
mod crates;
mod db;
mod events;
mod schema;
mod state_machine;
mod tags;

use crate::db::DB;
use crate::schema::{messages, roles, users};
use commands::{Args, Commands};
use diesel::prelude::*;
use events::{EventDispatcher, MessageDispatcher};
use serenity::{model::prelude::*, utils::parse_username, Client};
use std::str::FromStr;

type Result = crate::commands::Result<()>;

fn init_data() -> Result {
    info!("Loading data into database");
    let mod_role = std::env::var("MOD_ID").map_err(|_| "MOD_ID env var not found")?;
    let talk_role = std::env::var("TALK_ID").map_err(|_| "TALK_ID env var not found")?;
    let wg_and_teams_role =
        std::env::var("WG_AND_TEAMS_ID").map_err(|_| "WG_AND_TEAMS_ID env var not found")?;

    let conn = DB.get()?;

    let upsert_role = |name: &str, role_id: &str| -> Result {
        diesel::insert_into(roles::table)
            .values((roles::role.eq(role_id), roles::name.eq(name)))
            .on_conflict(roles::name)
            .do_update()
            .set(roles::role.eq(role_id))
            .execute(&conn)?;

        Ok(())
    };

    let _ = conn
        .build_transaction()
        .read_write()
        .run::<_, Box<dyn std::error::Error>, _>(|| {
            upsert_role("mod", &mod_role)?;
            upsert_role("talk", &talk_role)?;
            upsert_role("wg_and_teams", &wg_and_teams_role)?;

            Ok(())
        })?;

    Ok(())
}

fn app() -> Result {
    info!("starting...");
    let token = std::env::var("DISCORD_TOKEN")
        .map_err(|_| "missing environment variable: DISCORD_TOKEN")?;

    let _ = db::run_migrations()?;

    let _ = init_data()?;

    let mut cmds = Commands::new();

    // Tags
    cmds.add("?tags delete {key}", tags::delete);
    cmds.add("?tags create {key} value...", tags::post);
    cmds.add("?tags help", tags::help);
    cmds.add("?tags", tags::get_all);
    cmds.add("?tag {key}", tags::get);

    // crates.io
    cmds.add("?crate help", crates::help);
    cmds.add("?crate query...", crates::search);

    // docs.rs
    cmds.add("?docs help", crates::doc_help);
    cmds.add("?docs query...", crates::doc_search);

    // Slow mode.
    // 0 seconds disables slowmode
    cmds.add("?slowmode {channel} {seconds}", slow_mode);

    // Kick
    cmds.add("?kick {user}", kick);

    // Ban
    cmds.add("?ban {user}", ban);

    // Post the welcome message to the welcome channel.
    cmds.add("?CoC {channel}", welcome_message);

    let messages = MessageDispatcher::new(cmds);

    let mut client =
        Client::new_with_handlers(&token, Some(messages), Some(EventDispatcher)).unwrap();

    client.start()?;

    Ok(())
}

fn main() {
    env_logger::init();

    if let Err(err) = app() {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}

/// Set slow mode for a channel.  
///
/// A `seconds` value of 0 will disable slowmode
fn slow_mode(args: Args) -> Result {
    if api::is_mod(&args)? {
        let seconds = &args
            .params
            .get("seconds")
            .ok_or("unable to retrieve seconds param")?
            .parse::<u64>()?;

        let channel_name = &args
            .params
            .get("channel")
            .ok_or("unable to retrieve channel param")?;

        info!("Applying slowmode to channel {}", &channel_name);
        ChannelId::from_str(channel_name)?.edit(&args.cx, |c| c.slow_mode_rate(*seconds))?;
    }
    Ok(())
}

/// Kick a user from the guild.  
///
/// Requires the kick members permission
fn kick(args: Args) -> Result {
    if api::is_mod(&args)? {
        let user_id = parse_username(
            &args
                .params
                .get("user")
                .ok_or("unable to retrieve user param")?,
        )
        .ok_or("unable to retrieve user id")?;

        if let Some(guild) = args.msg.guild(&args.cx) {
            info!("Kicking user from guild");
            guild.read().kick(&args.cx, UserId::from(user_id))?
        }
    }
    Ok(())
}

/// Ban an user from the guild.  
///
/// Requires the ban members permission
fn ban(args: Args) -> Result {
    if api::is_mod(&args)? {
        let user_id = parse_username(
            &args
                .params
                .get("user")
                .ok_or("unable to retrieve user param")?,
        )
        .ok_or("unable to retrieve user id")?;

        if let Some(guild) = args.msg.guild(&args.cx) {
            info!("Banning user from guild");
            guild.read().ban(args.cx, UserId::from(user_id), &"all")?
        }
    }
    Ok(())
}

/// Write the welcome message to the welcome channel.  
fn welcome_message(args: Args) -> Result {
    const WELCOME_BILLBOARD: &'static str = "By participating in this community, you agree to follow the Rust Code of Conduct, as linked below. Please click the :white_check_mark: below to acknowledge and gain access to the channels.

  https://www.rust-lang.org/policies/code-of-conduct  ";

    if api::is_mod(&args)? {
        let channel_name = &args
            .params
            .get("channel")
            .ok_or("unable to retrieve channel param")?;

        let channel_id = ChannelId::from_str(channel_name)?;
        info!("Posting welcome message");
        let message = channel_id.say(&args.cx, WELCOME_BILLBOARD)?;
        let bot_id = &message.author.id;

        let conn = DB.get()?;

        let _ = conn
            .build_transaction()
            .read_write()
            .run::<_, Box<dyn std::error::Error>, _>(|| {
                let message_id = message.id.0.to_string();
                let channel_id = channel_id.0.to_string();

                diesel::insert_into(messages::table)
                    .values((
                        messages::name.eq("welcome"),
                        messages::message.eq(&message_id),
                        messages::channel.eq(&channel_id),
                    ))
                    .on_conflict(messages::name)
                    .do_update()
                    .set((
                        messages::message.eq(&message_id),
                        messages::channel.eq(&channel_id),
                    ))
                    .execute(&conn)?;

                let user_id = &bot_id.to_string();

                diesel::insert_into(users::table)
                    .values((users::user_id.eq(user_id), users::name.eq("me")))
                    .on_conflict(users::name)
                    .do_update()
                    .set((users::name.eq("me"), users::user_id.eq(user_id)))
                    .execute(&conn)?;
                Ok(())
            })?;

        let white_check_mark = ReactionType::from("✅");
        message.react(args.cx, white_check_mark)?;
    }
    Ok(())
}
