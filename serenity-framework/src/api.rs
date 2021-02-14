use crate::{Args, CommandHistory, Error};
use serenity::model::prelude::*;

/// Send a reply to the channel the message was received on.  
pub fn send_reply(args: &Args, message: &str) -> Result<(), Error> {
    if let Some(response_id) = response_exists(args) {
        log::info!("editing message: {:?}", response_id);
        args.msg
            .channel_id
            .edit_message(&args.ctx, response_id, |msg| msg.content(message))?;
    } else {
        let response = args.msg.channel_id.say(&args.ctx, message)?;

        let mut data = args.ctx.data.write();
        let history = data.get_mut::<CommandHistory>().unwrap();
        history.insert(args.msg.id, response.id);
    }

    Ok(())
}

fn response_exists(args: &Args) -> Option<MessageId> {
    let data = args.ctx.data.read();
    let history = data.get::<CommandHistory>().unwrap();
    history.get(&args.msg.id).copied()
}
