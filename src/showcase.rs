use crate::{Context, Error};
use poise::serenity_prelude as serenity;

fn create_embed<'a>(
    f: &'a mut serenity::CreateEmbed,
    author: &serenity::User,
    name: &str,
    description: &str,
    links: &str,
) -> &'a mut serenity::CreateEmbed {
    f.title(&name)
        .description(&description)
        .field("Links", &links, false)
        .author(|f| {
            if let Some(avatar_url) = author.avatar_url() {
                f.icon_url(avatar_url);
            }
            f.name(&author.name)
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
#[poise::command(prefix_command, slash_command)]
pub async fn showcase(ctx: Context<'_>) -> Result<(), Error> {
    let ask_the_user = |query| async move {
        poise::say_reply(ctx, format!("Please enter {}:", query)).await?;
        let user_input = ctx
            .author()
            .await_reply(ctx.discord())
            .channel_id(ctx.channel_id())
            .timeout(std::time::Duration::from_secs(10 * 60))
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

    poise::say_reply(
        ctx,
        format!(
            "Answer the following prompts to generate a <#{0}> entry. If you change your mind \
            later, you can edit or delete your messages, and the <#{0}> entry will be edited \
            or deleted accordingly.",
            ctx.data().showcase_channel.0
        ),
    )
    .await?;

    let name = ask_the_user("the name of your project").await?;
    let description = ask_the_user("a description of what the project is about").await?;
    let links =
        ask_the_user("URLs related to your project, like a crates.io or repository link").await?;

    let showcase_msg = ctx
        .data()
        .showcase_channel
        .send_message(ctx.discord(), |f| {
            f.allowed_mentions(|f| f).embed(|f| {
                create_embed(
                    f,
                    ctx.author(),
                    &name.content,
                    &description.content,
                    &links.content,
                )
            })
        })
        .await?;

    // TODO: Use ChannelId::create_public_thread once that's available
    if let Err(e) = ctx
        .discord()
        .http
        .create_public_thread(
            showcase_msg.channel_id.0,
            showcase_msg.id.0,
            &std::iter::FromIterator::from_iter(std::iter::once((
                String::from("name"),
                serde_json::Value::String(name.content.clone()),
            ))),
        )
        .await
    {
        println!(
            "Couldn't create associated thread for showcase entry: {}",
            e
        )
    }

    {
        let output_message = showcase_msg.id.0 as i64;
        let output_channel = showcase_msg.channel_id.0 as i64;
        let input_channel = ctx.channel_id().0 as i64;
        let name_input_message = name.id.0 as i64;
        let description_input_message = description.id.0 as i64;
        let links_input_message = links.id.0 as i64;
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

    poise::say_reply(
        ctx,
        format!(
            "Your project was successfully posted in <#{}>",
            ctx.data().showcase_channel.0
        ),
    )
    .await?;

    Ok(())
}

pub async fn try_update_showcase_message(
    ctx: &serenity::Context,
    data: &crate::Data,
    updated_message_id: serenity::MessageId,
) -> Result<(), Error> {
    let man = updated_message_id.0 as i64;
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
        let input_channel = serenity::ChannelId(entry.input_channel as u64);
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

        serenity::ChannelId(entry.output_channel as u64).edit_message(
            ctx,
            entry.output_message as u64,
            |f| f.embed(|f| create_embed(f, &name_msg.author, &name, &description, &links)),
        ).await?;
    }

    Ok(())
}

pub async fn try_delete_showcase_message(
    ctx: &serenity::Context,
    data: &crate::Data,
    deleted_message_id: serenity::MessageId,
) -> Result<(), Error> {
    let deleted_message_id = deleted_message_id.0 as i64;
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
        serenity::ChannelId(entry.output_channel as u64).delete_message(ctx, entry.output_message as u64).await?;
    }

    Ok(())
}
