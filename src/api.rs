use crate::{
    cache::RoleIdCache,
    commands::{Args, Result},
};
use serenity::model::prelude::*;

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
    use std::str::FromStr;

    if let Some((_, role_id, _)) = RoleIdCache::get_by_name("mod")? {
        Ok(has_role(args, &RoleId::from(u64::from_str(&role_id)?))?)
    } else {
        Ok(false)
    }
}
