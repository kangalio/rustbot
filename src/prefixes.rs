use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use serenity::futures::{StreamExt, TryStreamExt};

fn prefixes_explanation_text() -> String {
    "\
You don't want to be constrained to `?` or the good old \"hey ferris\"? Whatever cool prefixes \
you can think of, add them with `?prefix add your prefix here ` and you can use them to call \
the bot.

If your idea turns out less funny than you thought it would be, remove it with \
`?prefix remove your prefix here `.
    
Forgot your prefixes? Try `?prefix list`."
        .into()
}

/// Add custom user-specific prefixes
#[poise::command(
    prefix_command,
    slash_command,
    explanation_fn = "prefixes_explanation_text",
    category = "Miscellaneous"
)]
pub async fn prefix(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(prefixes_explanation_text()).await?;
    Ok(())
}

/// Add a new user-specific prefix that only you can use to invoke the bot.
#[poise::command(rename = "add", prefix_command, slash_command)]
pub async fn prefix_add(
    ctx: Context<'_>,
    #[description = "Prefix string to add"]
    #[rest]
    new_prefix: String,
) -> Result<(), Error> {
    let user_id = ctx.author().id.0 as i64;
    sqlx::query!(
        "INSERT INTO prefix (string, user_id) VALUES (?, ?)",
        new_prefix,
        user_id,
    )
    .execute(&ctx.data().database)
    .await?;

    ctx.say(format!("You can now use `{}` to speak to me!", new_prefix))
        .await?;

    Ok(())
}

async fn autocomplete_prefix(ctx: Context<'_>, partial: String) -> Vec<String> {
    let user_id = ctx.author().id.0 as i64;
    let prefixes = sqlx::query!("SELECT string FROM prefix WHERE user_id = ?", user_id)
        .fetch_many(&ctx.data().database);

    prefixes
        .filter_map(|result| async move { result.ok()?.right() })
        .map(|record| record.string)
        .filter(move |prefix| std::future::ready(prefix.starts_with(&partial)))
        .take(25)
        .collect()
        .await
}

/// Removes one of your user-specific prefixes that was added with `/prefix add`
#[poise::command(rename = "remove", prefix_command, slash_command)]
pub async fn prefix_remove(
    ctx: Context<'_>,
    #[description = "Prefix string to remove"]
    #[rest]
    #[autocomplete = "autocomplete_prefix"]
    prefix: String,
) -> Result<(), Error> {
    let user_id = ctx.author().id.0 as i64;
    let num_deleted_rows = sqlx::query!(
        "DELETE FROM prefix WHERE user_id = ? AND string = ?",
        user_id,
        prefix,
    )
    .execute(&ctx.data().database)
    .await?
    .rows_affected();

    let msg = if num_deleted_rows == 0 {
        format!("Cannot find `{}` in your prefixes", prefix)
    } else {
        format!("Removed `{}` from your prefixes", prefix)
    };
    ctx.say(msg).await?;

    Ok(())
}

/// Delete all of your user-specific prefixes
#[poise::command(rename = "reset", prefix_command, slash_command)]
pub async fn prefix_reset(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id.0 as i64;
    let prefixes_deleted = sqlx::query!("DELETE FROM prefix WHERE user_id = ?", user_id)
        .execute(&ctx.data().database)
        .await?
        .rows_affected();

    let msg = if prefixes_deleted == 0 {
        "You have no prefixes to delete".to_string()
    } else if prefixes_deleted == 1 {
        "1 prefix was deleted".to_string()
    } else {
        format!("{} prefixes were deleted", prefixes_deleted)
    };
    ctx.say(msg).await?;

    Ok(())
}

/// List all prefixes you configured for yourself
#[poise::command(rename = "list", prefix_command, slash_command)]
pub async fn prefix_list(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id.0 as i64;
    let mut prefixes = sqlx::query!("SELECT string FROM prefix WHERE user_id = ?", user_id)
        .fetch_many(&ctx.data().database);

    let mut response = format!("Prefixes configured for {}:\n", &ctx.author().name);
    while let Ok(Some(database_result)) = prefixes.try_next().await {
        if let Some(prefix) = database_result.right() {
            response += &format!("- `{}`\n", prefix.string);
        }
    }

    ctx.say(response).await?;

    Ok(())
}

pub async fn try_strip_prefix<'a>(
    _: &'a serenity::Context,
    msg: &'a serenity::Message,
    data: &'a crate::Data,
) -> Option<(&'a str, &'a str)> {
    let user_id = msg.author.id.0 as i64;
    let mut prefixes = sqlx::query!("SELECT string FROM prefix WHERE user_id = ?", user_id)
        .fetch_many(&data.database);

    while let Ok(Some(database_result)) = prefixes.try_next().await {
        if let Some(prefix) = database_result.right() {
            if msg.content.starts_with(&prefix.string) {
                return Some(msg.content.split_at(prefix.string.len()));
            }
        }
    }

    None
}
