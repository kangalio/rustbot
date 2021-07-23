use super::{api::*, util::*};
use crate::{Error, PrefixContext};

// play and eval work similarly, so this function abstracts over the two
async fn play_or_eval(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    force_warnings: bool, // If true, force enable warnings regardless of flags
    code: poise::CodeBlock,
    result_handling: ResultHandling,
) -> Result<(), Error> {
    let code = maybe_wrap(&code.code, result_handling);
    let (mut flags, flag_parse_errors) = parse_flags(flags);

    if force_warnings {
        flags.warn = true;
    }

    let mut result: PlayResult = ctx
        .data
        .http
        .post("https://play.rust-lang.org/execute")
        .json(&PlaygroundRequest {
            code: &code,
            channel: if let Edition::E2021 = flags.edition {
                // Edition 2021 only makes sense with nightly at the moment
                Channel::Nightly
            } else {
                flags.channel
            },
            crate_type: if code.contains("fn main") {
                CrateType::Binary
            } else {
                CrateType::Library
            },
            edition: flags.edition,
            mode: flags.mode,
            tests: false,
        })
        .send()
        .await?
        .json()
        .await?;

    result.stderr = format_play_eval_stderr(&result.stderr, flags.warn);

    send_reply(ctx, result, &code, &flags, &flag_parse_errors).await
}

/// Compile and run Rust code in a playground
#[poise::command(track_edits, broadcast_typing, explanation_fn = "play_help")]
pub async fn play(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    play_or_eval(ctx, flags, false, code, ResultHandling::None).await
}

pub fn play_help() -> String {
    generic_help(GenericHelp {
        command: "play",
        desc: "Compile and run Rust code",
        mode_and_channel: true,
        warn: true,
        example_code: "code",
    })
}

/// Compile and run Rust code with warnings
#[poise::command(
    track_edits,
    broadcast_typing,
    hide_in_help, // don't clutter help menu with something that ?play can do too
    explanation_fn = "playwarn_help"
)]
pub async fn playwarn(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    play_or_eval(ctx, flags, true, code, ResultHandling::None).await
}

pub fn playwarn_help() -> String {
    generic_help(GenericHelp {
        command: "playwarn",
        desc: "Compile and run Rust code with warnings. Equivalent to `?play warn=true`",
        mode_and_channel: true,
        warn: false,
        example_code: "code",
    })
}

/// Evaluate a single Rust expression
#[poise::command(track_edits, broadcast_typing, explanation_fn = "eval_help")]
pub async fn eval(
    ctx: PrefixContext<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    play_or_eval(ctx, flags, false, code, ResultHandling::Print).await
}

pub fn eval_help() -> String {
    generic_help(GenericHelp {
        command: "eval",
        desc: "Compile and run Rust code",
        mode_and_channel: true,
        warn: true,
        example_code: "code",
    })
}
