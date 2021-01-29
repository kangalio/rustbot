use crate::{
    api, commands::Args, db::DB, schema::bans, text::ban_message, Error, SendSyncError, HOUR,
};
use diesel::prelude::*;
use serenity::{model::prelude::*, prelude::*, utils::parse_username};
use std::time::{Duration, SystemTime};

pub fn save_ban(user_id: String, guild_id: String, hours: u64) -> Result<(), Error> {
    info!("Recording ban for user {}", &user_id);
    let conn = DB.get()?;
    diesel::insert_into(bans::table)
        .values((
            bans::user_id.eq(user_id),
            bans::guild_id.eq(guild_id),
            bans::start_time.eq(SystemTime::now()),
            bans::end_time.eq(SystemTime::now()
                .checked_add(Duration::new(hours * HOUR, 0))
                .ok_or("out of range Duration for ban end_time")?),
        ))
        .execute(&conn)?;

    Ok(())
}

pub fn save_unban(user_id: String, guild_id: String) -> Result<(), Error> {
    info!("Recording unban for user {}", &user_id);
    let conn = DB.get()?;
    diesel::update(bans::table)
        .filter(
            bans::user_id
                .eq(user_id)
                .and(bans::guild_id.eq(guild_id).and(bans::unbanned.eq(false))),
        )
        .set(bans::unbanned.eq(true))
        .execute(&conn)?;

    Ok(())
}

pub fn unban_users(cx: &Context) -> Result<(), SendSyncError> {
    use std::str::FromStr;

    let conn = DB.get()?;
    let to_unban = bans::table
        .filter(
            bans::unbanned
                .eq(false)
                .and(bans::end_time.le(SystemTime::now())),
        )
        .load::<(i32, String, String, bool, SystemTime, SystemTime)>(&conn)?;

    for row in &to_unban {
        let guild_id = GuildId::from(u64::from_str(&row.2)?);
        info!("Unbanning user {}", &row.1);
        guild_id.unban(&cx, u64::from_str(&row.1)?)?;
    }
    Ok(())
}

/// Temporarily ban an user from the guild.  
///
/// Requires the ban members permission
pub fn temp_ban(args: Args) -> Result<(), Error> {
    let user_id = parse_username(
        &args
            .params
            .get("user")
            .ok_or("unable to retrieve user param")?,
    )
    .ok_or("unable to retrieve user id")?;

    use std::str::FromStr;

    let hours = u64::from_str(
        args.params
            .get("hours")
            .ok_or("unable to retrieve hours param")?,
    )?;

    let reason = args
        .params
        .get("reason")
        .ok_or("unable to retrieve reason param")?;

    if let Some(guild) = args.msg.guild(&args.cx) {
        info!("Banning user from guild");
        let user = UserId::from(user_id);

        user.create_dm_channel(args.cx)?
            .say(args.cx, ban_message(reason, hours))?;

        guild.read().ban(args.cx, &user, &"all")?;

        save_ban(
            format!("{}", user_id),
            format!("{}", guild.read().id),
            hours,
        )?;
    }
    Ok(())
}

pub fn help(args: Args) -> Result<(), Error> {
    let hours = 24;
    let reason = "violating the code of conduct";

    let help_string = format!(
        "
Ban a user for a temporary amount of time
```
{command}
```
**Example:**
```
?ban @someuser {hours} {reason}
```
will ban a user for {hours} hours and send them the following message:
```
{user_message}
```
",
        command = "?ban {user} {hours} reason...",
        user_message = ban_message(reason, hours),
        hours = hours,
        reason = reason,
    );

    api::send_reply(&args, &help_string)?;
    Ok(())
}
