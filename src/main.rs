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
mod welcome;

use crate::db::DB;
use commands::{Args, Commands};
use diesel::prelude::*;
use events::{EventDispatcher, MessageDispatcher};
use serenity::Client;

pub(crate) type Result = crate::commands::Result<()>;

fn init_data() -> Result {
    use crate::schema::roles;
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
    cmds.add("?slowmode {channel} {seconds}", api::slow_mode);

    // Kick
    cmds.add("?kick {user}", api::kick);

    // Ban
    cmds.add("?ban {user}", api::ban);

    // Post the welcome message to the welcome channel.
    cmds.add("?CoC {channel}", welcome::post_message);

    let menu = cmds.menu().unwrap();

    cmds.add("?help", move |args: Args| {
        api::send_reply(&args, &format!("```{}```", &menu))?;
        Ok(())
    });

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
