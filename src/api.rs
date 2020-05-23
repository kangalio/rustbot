use crate::commands::{Args, Result};
use crate::db::DB;
use crate::schema::roles;
use diesel::prelude::*;
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
