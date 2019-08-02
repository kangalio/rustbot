use crate::commands::{Args, Result};

/// Send a reply to the channel the message was received on.  
pub(crate) fn send_reply(args: &Args, message: &str) -> Result<()> {
    args.msg.channel_id.say(&args.cx, message)?;
    Ok(())
}

/// Return whether or not the user is a mod.  
pub(crate) fn is_mod(args: &Args) -> Result<bool> {
    let guild = args.msg.guild(&args.cx).ok_or("Unable to fetch guild")?;

    let role = guild
        .read()
        .role_by_name("mod")
        .ok_or("Unable to fetch role")?
        .id;

    Ok(args
        .msg
        .member
        .as_ref()
        .ok_or("Unable to fetch member")?
        .roles
        .contains(&role))
}
