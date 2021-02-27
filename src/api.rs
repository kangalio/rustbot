use crate::{Context, Error};

use serenity::model::prelude::*;

/// Send a reply to the channel the message was received on.  
pub async fn send_reply(
    ctx: impl ContextAndData<crate::Data>,
    trigger: &Message,
    response: &str,
) -> Result<(), Error> {
    if let Some(response_id) = ctx
        .data()
        .command_history
        .lock()
        .await
        .get(&trigger.id)
        .copied()
    {
        info!("editing message: {:?}", response_id);
        trigger
            .channel_id
            .edit_message(&ctx.ctx(), response_id, |msg| msg.content(response))
            .await?;
    } else {
        let response = trigger.channel_id.say(&ctx.ctx(), response).await?;
        let mut history = ctx.data().command_history.lock().await;
        history.insert(trigger.id, response.id);
    }

    Ok(())
}

pub trait ContextAndData<D> {
    fn ctx(&self) -> &serenity::prelude::Context;
    fn data(&self) -> &D;
}

impl<D> ContextAndData<D> for &serenity_framework::context::Context<D> {
    fn ctx(&self) -> &serenity::prelude::Context {
        &self.serenity_ctx
    }

    fn data(&self) -> &D {
        &*self.data
    }
}

impl<D> ContextAndData<D> for (&serenity::prelude::Context, &D) {
    fn ctx(&self) -> &serenity::prelude::Context {
        self.0
    }

    fn data(&self) -> &D {
        self.1
    }
}
