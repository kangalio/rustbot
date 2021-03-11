use crate::{
    command_history::{CommandHistory, CommandHistoryEntry},
    commands::Args,
    Error,
};
use serenity::model::prelude::*;

/// Send a reply to the channel the message was received on.  
pub fn send_reply(args: &Args, message: &str) -> Result<(), Error> {
    if let Some(response_id) = find_response(args) {
        info!("editing message: {:?}", response_id);
        args.msg
            .channel_id
            .edit_message(&args.cx, response_id, |msg| msg.content(message))?;
    } else {
        let response = args.msg.channel_id.say(&args.cx, message)?;

        let mut data = args.cx.data.write();
        let history = data.get_mut::<CommandHistory>().unwrap();
        history.push(CommandHistoryEntry {
            user_message: args.msg.clone(),
            response,
        });
    }

    Ok(())
}

fn find_response(args: &Args) -> Option<MessageId> {
    let data = args.cx.data.read();
    let history = data.get::<CommandHistory>().unwrap();
    history
        .iter()
        .find(|entry| entry.user_message.id == args.msg.id)
        .map(|entry| entry.response.id)
}
