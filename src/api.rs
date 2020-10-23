use crate::{
    commands::{Args, Result},
    db::DB,
    schema::roles,
};
use diesel::prelude::*;
use serenity::{model::prelude::*, utils::parse_username};

/// Send a reply to the channel the message was received on.  
pub(crate) fn send_reply(args: &Args, message: &str) -> Result<()> {
    args.msg.channel_id.say(&args.cx, message)?;
    Ok(())
}

/// Determine if a member sending a message has the `Role`.  
pub(crate) fn has_role(args: &Args, role: &RoleId) -> Result<bool> {
    Ok(args
        .msg
        .member
        .as_ref()
        .ok_or("Unable to fetch member")?
        .roles
        .contains(role))
}

fn check_permission(args: &Args, role: Option<String>) -> Result<bool> {
    use std::str::FromStr;
    if let Some(role_id) = role {
        Ok(has_role(args, &RoleId::from(u64::from_str(&role_id)?))?)
    } else {
        Ok(false)
    }
}

/// Return whether or not the user is a mod.  
pub(crate) fn is_mod(args: &Args) -> Result<bool> {
    let role = roles::table
        .filter(roles::name.eq("mod"))
        .first::<(i32, String, String)>(&DB.get()?)
        .optional()?;

    check_permission(args, role.map(|(_, role_id, _)| role_id))
}

pub(crate) fn is_wg_and_teams(args: &Args) -> Result<bool> {
    let role = roles::table
        .filter(roles::name.eq("wg_and_teams"))
        .first::<(i32, String, String)>(&DB.get()?)
        .optional()?;

    check_permission(args, role.map(|(_, role_id, _)| role_id))
}

/// Set slow mode for a channel.  
///
/// A `seconds` value of 0 will disable slowmode
pub(crate) fn slow_mode(args: Args) -> Result<()> {
    use std::str::FromStr;

    if is_mod(&args)? {
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

pub(crate) fn slow_mode_help(args: Args) -> Result<()> {
    let help_string = "
Set slowmode on a channel
```
?slowmode {channel} {seconds}
```
**Example:**
```
?slowmode #bot-usage 10
```
will set slowmode on the `#bot-usage` channel with a delay of 10 seconds.  

**Disable slowmode:**
```
?slowmode #bot-usage 0
```
will disable slowmode on the `#bot-usage` channel.";
    send_reply(&args, &help_string)?;
    Ok(())
}

/// Kick a user from the guild.  
///
/// Requires the kick members permission
pub(crate) fn kick(args: Args) -> Result<()> {
    if is_mod(&args)? {
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

pub(crate) fn kick_help(args: Args) -> Result<()> {
    let help_string = "
Kick a user from the guild
```
?kick {user}
```
**Example:**
```
?kick @someuser
```
will kick a user from the guild.";
    send_reply(&args, &help_string)?;
    Ok(())
}
