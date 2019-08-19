use crate::{
    cache::RoleIdCache,
    commands::{Args, Result},
};
use serenity::{model::prelude::*};

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

/// Return whether or not the user is a mod.  
pub(crate) fn is_mod(args: &Args) -> Result<bool> {
    let data = args.cx.data.read();

    let role_store = data
        .get::<RoleIdCache>()
        .ok_or("Unable to fetch RoleIdCache")?;

    let mod_role = role_store
        .get("mod".into())
        .ok_or("Unable to retrieve mod role from cache")?;

    Ok(has_role(args, mod_role)?)
}
