use crate::{serenity, Context, Error};

async fn check_is_moderator(ctx: Context<'_>) -> Result<bool, Error> {
    // Retrieve via HTTP to make sure it's up-to-date
    let author = ctx
        .discord()
        .http
        .get_member(
            ctx.guild_id()
                .ok_or("This command only works inside guilds")?,
            ctx.author().id,
        )
        .await?;

    Ok(if author.roles.contains(&ctx.data().mod_role_id) {
        true
    } else {
        ctx.send(
            poise::CreateReply::new()
                .content("This command is only available to moderators")
                .ephemeral(true),
        )
        .await?;
        false
    })
}

async fn immediately_lift_slowmode(ctx: Context<'_>) -> Result<(), Error> {
    let active_slowmode = ctx
        .data()
        .active_slowmodes
        .lock()
        .unwrap()
        .remove(&ctx.channel_id());

    match active_slowmode {
        Some(active_slowmode) => {
            ctx.channel_id()
                .edit(
                    ctx.discord(),
                    serenity::EditChannel::new()
                        .rate_limit_per_user(active_slowmode.previous_slowmode_rate),
                )
                .await?;
            ctx.say("Restored slowmode to previous level").await?;
        }
        None => {
            ctx.say("There is no slowmode command currently running")
                .await?;
        }
    }

    Ok(())
}

async fn register_slowmode(
    ctx: Context<'_>,
    duration_argument: Option<u64>,
    rate_argument: Option<u64>,
) -> Result<(u64, u64), Error> {
    let current_slowmode_rate = match ctx.channel_id().to_channel(ctx.discord()).await {
        Ok(channel) => channel
            .guild()
            .ok_or("This command only works inside guilds")?
            .rate_limit_per_user
            .unwrap_or(0),
        Err(e) => {
            log::warn!("Couldn't retrieve channel slowmode settings: {}", e);
            0
        }
    };

    let mut active_slowmodes = ctx.data().active_slowmodes.lock().unwrap();
    let already_active_slowmode = active_slowmodes.get(&ctx.channel_id());

    // If we're overwriting an existing slowmode command, the channel's current slowmode rate
    // is not the original one, so we check the existing entry
    let previous_slowmode_rate =
        already_active_slowmode.map_or(current_slowmode_rate, |s| s.previous_slowmode_rate);
    let duration = duration_argument
        .or_else(|| Some(already_active_slowmode?.duration))
        .unwrap_or(30);
    let rate = rate_argument
        .or_else(|| Some(already_active_slowmode?.rate))
        .unwrap_or(15);

    active_slowmodes.insert(
        ctx.channel_id(),
        crate::ActiveSlowmode {
            previous_slowmode_rate,
            duration,
            rate,
            invocation_time: *ctx.created_at(),
        },
    );

    Ok((duration, rate))
}

async fn restore_slowmode_rate(ctx: Context<'_>) -> Result<(), Error> {
    let previous_slowmode_rate = {
        let active_slowmodes = &ctx.data().active_slowmodes;
        let active_slowmode = active_slowmodes.lock().unwrap().remove(&ctx.channel_id());
        let active_slowmode = match active_slowmode {
            Some(x) => x,
            None => {
                log::info!(
                    "Slowmode entry has expired; this slowmode invocation has been overwritten"
                );
                return Ok(());
            }
        };
        if active_slowmode.invocation_time != *ctx.created_at() {
            log::info!(
                "Slowmode entry has a different invocation time; \
                this slowmode invocation has been overwritten"
            );
            return Ok(());
        }
        active_slowmode.previous_slowmode_rate
    };

    log::info!("Restoring slowmode rate to {}", previous_slowmode_rate);
    ctx.channel_id()
        .edit(
            ctx.discord(),
            serenity::EditChannel::new().rate_limit_per_user(previous_slowmode_rate),
        )
        .await?;
    ctx.data()
        .active_slowmodes
        .lock()
        .unwrap()
        .remove(&ctx.channel_id());

    Ok(())
}

/// Temporarily enables slowmode for this channel (moderator only)
///
/// After the specified duration, the slowmode will be reset to previous level. Invoke the command \
/// with duration set to zero to immediately lift slowmode. If the command is invoked while an
/// existing invocation is running, the running invocation will be overwritten.
///
/// Default duration: 30 minutes
/// Default rate: 15 seconds
#[poise::command(slash_command, prefix_command, hide_in_help, category = "Moderation")]
pub async fn slowmode(
    ctx: Context<'_>,
    #[description = "How long slowmode should persist for this channel, in minutes"]
    duration: Option<u64>, // TODO: make f32 with a #[min = 0.0] attribute (once poise supports it)
    #[description = "How many seconds a user has to wait before sending another message (0-120)"]
    rate: Option<u64>,
) -> Result<(), Error> {
    if !check_is_moderator(ctx).await? {
        return Ok(());
    }

    if duration == Some(0) || rate == Some(0) {
        immediately_lift_slowmode(ctx).await?;
        return Ok(());
    }

    // Register that there is an active slowmode command, or overwrite an existing entry.
    // In the end, we can make sure that our slowmode command invocation has not been overwritten
    // since by a new invocation
    let (duration, rate) = register_slowmode(ctx, duration, rate).await?;

    // Apply slowmode
    ctx.channel_id()
        .edit(
            ctx.discord(),
            serenity::EditChannel::new().rate_limit_per_user(rate),
        )
        .await?;

    // Confirmation message
    let _: Result<_, _> = ctx
        .say(format!(
            "Slowmode will be enabled for {} minutes. \
            Members can send one message every {} seconds",
            duration, rate,
        ))
        .await;

    // Wait until slowmode is over
    tokio::time::sleep(std::time::Duration::from_secs(60 * duration)).await;

    restore_slowmode_rate(ctx).await?;

    Ok(())
}
