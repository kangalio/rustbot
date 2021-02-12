use crate::{command_history::CommandHistory, commands::Args, Error};
use serenity::model::prelude::*;

/// Send a reply to the channel the message was received on.  
pub fn send_reply(args: &Args, message: &str) -> Result<(), Error> {
    if let Some(response_id) = response_exists(args) {
        info!("editing message: {:?}", response_id);
        args.msg
            .channel_id
            .edit_message(&args.cx, response_id, |msg| msg.content(message))?;
    } else {
        let response = args.msg.channel_id.say(&args.cx, message)?;

        let mut data = args.cx.data.write();
        let history = data.get_mut::<CommandHistory>().unwrap();
        history.insert(args.msg.id, response.id);
    }

    Ok(())
}

fn response_exists(args: &Args) -> Option<MessageId> {
    let data = args.cx.data.read();
    let history = data.get::<CommandHistory>().unwrap();
    history.get(&args.msg.id).copied()
}
