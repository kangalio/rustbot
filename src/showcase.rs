use crate::{Context, Error};
use poise::serenity_prelude as serenity;

fn create_embed(
    author: &serenity::User,
    name: &str,
    description: &str,
    links: &str,
) -> serenity::CreateEmbed {
    serenity::CreateEmbed::new()
        .title(name)
        .description(description)
        .field("Links", links, false)
        .author({
            let mut b = serenity::CreateEmbedAuthor::new(&author.name);
            if let Some(avatar_url) = author.avatar_url() {
                b = b.icon_url(avatar_url);
            }
            b
        })
        .color(crate::EMBED_COLOR)
}

/// Asks details about your project and then posts it in the #showcase channel
///
/// Starts a prompt where you can enter information about a project you're working on. The bot \
/// will then post your project into the #showcase channel and open a thread to allow for \
/// discussion and feedback.
///
/// If you want to change the text later, edit your message and the bot will propagate the change.
/// You can also delete your message to delete the #showcase entry.
#[poise::command(prefix_command, slash_command, category = "Moderation")]
pub async fn showcase(ctx: Context<'_>) -> Result<(), Error> {
    let ask_the_user = |query| async move {
        ctx.say(format!("Please enter {}:", query)).await?;
        let user_input = ctx
            .author()
            .reply_collector(&ctx.discord().shard)
            .channel_id(ctx.channel_id())
            .timeout(std::time::Duration::from_secs(10 * 60))
            .collect_single()
            .await;

        let user_input = user_input.ok_or_else(|| {
            Error::from(format!(
                "You didn't enter {}. Please run the command again to restart",
                query
            ))
        })?;

        match user_input.content.to_ascii_lowercase().trim() {
            "abort" | "stop" | "cancel" | "break" | "terminate" | "exit" | "quit" => {
                return Err(Error::from("Canceled the operation"))
            }
            _ => {}
        }

        Ok(user_input)
    };

    ctx.say(format!(
        "Answer the following prompts to generate a <#{0}> entry. If you change your mind \
            later, you can edit or delete your messages to edit or delete the <#{0}> entry. To \
            cancel, type `cancel`",
        ctx.data().showcase_channel.0
    ))
    .await?;

    let name = ask_the_user("the name of your project").await?;
    let description = ask_the_user("a description of what the project is about").await?;
    let links =
        ask_the_user("URLs related to your project, like a crates.io or repository link").await?;

    let showcase_msg = ctx
        .data()
        .showcase_channel
        .send_message(
            ctx.discord(),
            serenity::CreateMessage::new()
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .embed(create_embed(
                    ctx.author(),
                    &name.content,
                    &description.content,
                    &links.content,
                )),
        )
        .await?;

    match showcase_msg
        .channel_id
        .create_public_thread(
            ctx.discord(),
            showcase_msg.id,
            serenity::CreateThread::new(&name.content),
        )
        .await
    {
        Ok(thread) => {
            if let Err(e) = ctx
                .discord()
                .http
                .add_thread_channel_member(thread.id, ctx.author().id)
                .await
            {
                log::warn!("Couldn't add member to showcase thread: {}", e);
            }
        }
        Err(e) => log::warn!(
            "Couldn't create associated thread for showcase entry: {}",
            e
        ),
    }

    {
        let output_message = showcase_msg.id.get() as i64;
        let output_channel = showcase_msg.channel_id.get() as i64;
        let input_channel = ctx.channel_id().get() as i64;
        let name_input_message = name.id.get() as i64;
        let description_input_message = description.id.get() as i64;
        let links_input_message = links.id.get() as i64;
        sqlx::query!(
            "INSERT INTO showcase (
                output_message,
                output_channel,
                input_channel,
                name_input_message,
                description_input_message,
                links_input_message
            ) VALUES (?, ?, ?, ?, ?, ?)",
            output_message,
            output_channel,
            input_channel,
            name_input_message,
            description_input_message,
            links_input_message,
        )
        .execute(&ctx.data().database)
        .await?;
    }

    ctx.say(format!(
        "Your project was successfully posted in <#{}>",
        ctx.data().showcase_channel.0
    ))
    .await?;

    Ok(())
}

pub async fn try_update_showcase_message(
    ctx: &serenity::Context,
    data: &crate::Data,
    updated_message_id: serenity::MessageId,
) -> Result<(), Error> {
    let man = updated_message_id.get() as i64;
    if let Some(entry) = sqlx::query!(
        "SELECT
            output_message,
            output_channel,
            input_channel,
            name_input_message,
            description_input_message,
            links_input_message
        FROM showcase WHERE ? IN (name_input_message, description_input_message, links_input_message)",
        man
    )
    .fetch_optional(&data.database)
    .await?
    {
        let input_channel = serenity::ChannelId::new(entry.input_channel as u64);
        let name_msg = input_channel
            .message(ctx, entry.name_input_message as u64)
            .await?;
        let name = &name_msg.content;
        let description = input_channel
            .message(ctx, entry.description_input_message as u64)
            .await?
            .content;
        let links = input_channel
            .message(ctx, entry.links_input_message as u64)
            .await?
            .content;

        serenity::ChannelId::new(entry.output_channel as u64).edit_message(
            ctx,
            entry.output_message as u64,
            serenity::EditMessage::new().embed(create_embed(&name_msg.author, name, &description, &links)),
        ).await?;
    }

    Ok(())
}

pub async fn try_delete_showcase_message(
    ctx: &serenity::Context,
    data: &crate::Data,
    deleted_message_id: serenity::MessageId,
) -> Result<(), Error> {
    let deleted_message_id = deleted_message_id.get() as i64;
    if let Some(entry) = sqlx::query!(
        "SELECT
            output_message,
            output_channel
        FROM showcase WHERE ? IN (name_input_message, description_input_message, links_input_message)",
        deleted_message_id
    )
    .fetch_optional(&data.database)
    .await?
    {
        serenity::ChannelId::new(entry.output_channel as u64).delete_message(ctx, entry.output_message as u64).await?;
    }

    Ok(())
}
