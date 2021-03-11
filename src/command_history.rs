use crate::{commands::Commands, Error};
use serenity::{model::prelude::*, prelude::*};

pub struct CommandHistory;

pub struct CommandHistoryEntry {
    pub user_message: Message,
    pub response: Message,
}

impl TypeMapKey for CommandHistory {
    type Value = Vec<CommandHistoryEntry>;
}

/// Decide if a message is old enough to throw it out from the cache
fn is_message_too_old(age: chrono::Duration) -> bool {
    age > chrono::Duration::minutes(60)
}

fn update_message(message: &mut Message, update: MessageUpdateEvent) {
    message.channel_id = update.channel_id;
    message.guild_id = update.guild_id;

    if let Some(kind) = update.kind {
        message.kind = kind;
    }

    if let Some(content) = update.content {
        message.content = content;
    }

    if let Some(tts) = update.tts {
        message.tts = tts;
    }

    if let Some(pinned) = update.pinned {
        message.pinned = pinned;
    }

    if let Some(timestamp) = update.timestamp {
        message.timestamp = timestamp;
    }

    if let Some(edited_timestamp) = update.edited_timestamp {
        message.edited_timestamp = Some(edited_timestamp);
    }

    if let Some(author) = update.author {
        message.author = author;
    }

    if let Some(mention_everyone) = update.mention_everyone {
        message.mention_everyone = mention_everyone;
    }

    if let Some(mentions) = update.mentions {
        message.mentions = mentions;
    }

    if let Some(mention_roles) = update.mention_roles {
        message.mention_roles = mention_roles;
    }

    if let Some(attachments) = update.attachments {
        message.attachments = attachments;
    }

    // if let Some(embeds) = update.embeds {
    //     message.embeds = embeds;
    // }
}

// Called whenever any message is edited by anyone
pub fn apply_message_update(cx: Context, update: MessageUpdateEvent, cmds: &Commands) {
    if let (Some(created), Some(edited)) = (update.timestamp, update.edited_timestamp) {
        if is_message_too_old(edited - created) {
            return;
        }
    } else {
        // Idk if this is correct, but the previous codebase did it like this
        return;
    }

    let mut data = cx.data.write();
    let history = data.get_mut::<CommandHistory>().unwrap();

    // Find the user message in cache or create a blank one, to be filled in with updates later
    let mut user_message = if let Some(history_entry) = history
        .iter_mut()
        .find(|entry| entry.user_message.id == update.id)
    {
        history_entry.user_message.clone()
    } else {
        // Construct a blank message that will be filled in later
        serenity::utils::CustomMessage::new().build()
    };

    drop(data); // avoid blocking

    update_message(&mut user_message, update);
    cmds.execute(&cx, &user_message);
}

pub fn clear_command_history(cx: &Context) -> Result<(), Error> {
    let mut data = cx.data.write();
    let history = data.get_mut::<CommandHistory>().unwrap();

    // always keep the last command in history
    if !history.is_empty() {
        info!("Clearing command history");
        history.drain(..history.len() - 1);
    }
    Ok(())
}
