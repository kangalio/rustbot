use crate::{command_history::CommandHistory, commands::Args, db::DB, schema::roles, Error};
use diesel::prelude::*;
use serenity::{model::prelude::*, utils::parse_username};

/// Send a reply to the channel the message was received on.  
pub fn send_reply(args: &Args, message: &str) -> Result<(), Error> {
    if let Some(response_id) = response_exists(args) {
        info!("editing message: {:?}", response_id);
        args.msg
            .channel_id
            .edit_message(&args.cx, response_id, |msg| msg.content(message))?;
    } else {
        let command_id = args.msg.id;
        let response = args.msg.channel_id.say(&args.cx, message)?;

        let mut data = args.cx.data.write();
        let history = data.get_mut::<CommandHistory>().unwrap();
        history.insert(command_id, response.id);
    }

    Ok(())
}

fn response_exists(args: &Args) -> Option<MessageId> {
    let data = args.cx.data.read();
    let history = data.get::<CommandHistory>().unwrap();
    history.get(&args.msg.id).cloned()
}

/// Determine if a member sending a message has the `Role`.  
pub fn has_role(args: &Args, role: &RoleId) -> Result<bool, Error> {
    Ok(args
        .msg
        .member
        .as_ref()
        .ok_or("Unable to fetch member")?
        .roles
        .contains(role))
}

fn check_permission(args: &Args, role: Option<String>) -> Result<bool, Error> {
    Ok(if let Some(role_id) = role {
        has_role(args, &role_id.parse::<u64>()?.into())?
    } else {
        false
    })
}

/// Return whether or not the user is a mod.  
pub fn is_mod(args: &Args) -> Result<bool, Error> {
    let role = roles::table
        .filter(roles::name.eq("mod"))
        .first::<(i32, String, String)>(&DB.get()?)
        .optional()?;

    check_permission(args, role.map(|(_, role_id, _)| role_id))
}

/// Set slow mode for a channel.  
///
/// A `seconds` value of 0 will disable slowmode
pub fn slow_mode(args: &Args) -> Result<(), Error> {
    if !is_mod(args)? {
        return Err(Error::MissingPermissions);
    }

    let mut token = args.body.splitn(2, ' ');
    let (seconds, channel) = match (token.next(), token.next()) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err("unable to retrieve seconds or channel param".into()),
    };

    let seconds = seconds.parse::<u64>()?;
    let channel = channel.parse::<ChannelId>()?;

    info!("Applying slowmode to channel {}", &channel);
    channel.edit(&args.cx, |c| c.slow_mode_rate(seconds))?;

    Ok(())
}

pub fn slow_mode_help(args: &Args) -> Result<(), Error> {
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
pub fn kick(args: &Args) -> Result<(), Error> {
    if is_mod(&args)? {
        let user_id = parse_username(args.body).ok_or("unable to retrieve user id")?;

        if let Some(guild) = args.msg.guild(&args.cx) {
            info!("Kicking user from guild");
            guild.read().kick(&args.cx, UserId::from(user_id))?
        }
    }
    Ok(())
}

pub fn kick_help(args: &Args) -> Result<(), Error> {
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
