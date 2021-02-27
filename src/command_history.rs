use crate::Error;
use serenity::{model::prelude::*, prelude::*, utils::CustomMessage};

pub async fn replay_message(
    ctx: serenity::prelude::Context,
    ev: MessageUpdateEvent,
    events: &crate::Events,
) -> Result<(), Error> {
    if let (Some(created), Some(edited)) = (ev.timestamp, ev.edited_timestamp) {
        // Only track edits for recent messages
        if (edited - created).num_minutes() < 60 {
            let mut msg = CustomMessage::new();
            msg.id(ev.id)
                .channel_id(ev.channel_id)
                .content(ev.content.unwrap_or_else(String::new));
            events.message(ctx, msg.build()).await;
        }
    }

    Ok(())
}

pub async fn clear_command_history(data: &crate::Data) {
    let mut history = data.command_history.lock().await;

    let last_entry = history.pop();
    history.clear();
    // always keep the last command in history
    if let Some(last_entry) = last_entry {
        history.insert(last_entry.0, last_entry.1);
    }
}
