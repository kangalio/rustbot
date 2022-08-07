use super::{api::*, util::*};
use crate::{Context, Error};

// We are not knocking ourselves out here.
const MIR_UNSTABLE_WARNING: &str = "// WARNING: This output format is intended for human consumers only\n// and is subject to change without notice. Knock yourself out.\n";

/// Show MIR for the code
#[poise::command(
    prefix_command,
    track_edits,
    help_text_fn = "mir_help",
    category = "Playground"
)]
pub async fn mir(
    ctx: Context<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    ctx.say(stub_message(ctx)).await?;

    let (flags, flag_parse_errors) = parse_flags(flags);

    let code = format!("#![allow(dead_code)]\n{}", code.code);

    let result: CompileResponse = ctx
        .data()
        .http
        .post("https://play.rust-lang.org/compile")
        .json(&CompileRequest {
            assembly_flavor: AssemblyFlavour::default(),
            backtrace: false,
            channel: flags.channel,
            code: &code,
            crate_type: CrateType::Library,
            demangle_assembly: DemangleAssembly::default(),
            edition: flags.edition,
            mode: flags.mode,
            process_assembly: ProcessAssembly::default(),
            target: CompileTarget::Mir,
            tests: false,
        })
        .send()
        .await?
        .json()
        .await?;

    // Strip boilerplates line and surgically remove the two useless warnings
    let stderr = format_play_eval_stderr(&result.stderr, flags.warn).replace("\
warning: due to multiple output types requested, the explicitly specified output file name will be adapted for each output type

warning: ignoring --out-dir flag due to -o flag
", "");

    // Remove two boilerplate lines off the start
    let stdout = result
        .code
        .trim_start_matches(MIR_UNSTABLE_WARNING)
        .to_owned();

    let result = PlayResult {
        stdout,
        stderr,
        success: result.success,
    };

    send_reply(ctx, result, &code, &flags, &flag_parse_errors).await
}

pub fn mir_help() -> String {
    generic_help(GenericHelp {
        command: "mir",
        desc: "Show MIR for code",
        mode_and_channel: false,
        warn: false,
        run: false,
        example_code: "code",
    })
}
