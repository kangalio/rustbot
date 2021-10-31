use super::{api::*, util::*};
use crate::{Context, Error};

const BENCH_FUNCTION: &str = r#"
fn bench(functions: &[(&str, fn())]) {
    const CHUNK_SIZE: usize = 10000;

    // Warm up
    for (_, function) in functions.iter() {
        for _ in 0..CHUNK_SIZE {
            (function)();
        }
    }

    let mut functions_chunk_times = functions.iter().map(|_| Vec::new()).collect::<Vec<_>>();

    let start = std::time::Instant::now();
    while (std::time::Instant::now() - start).as_secs() < 5 {
        for (chunk_times, (_, function)) in functions_chunk_times.iter_mut().zip(functions) {
            let start = std::time::Instant::now();
            for _ in 0..CHUNK_SIZE {
                (function)();
            }
            chunk_times.push((std::time::Instant::now() - start).as_secs_f64() / CHUNK_SIZE as f64);
        }
    }

    for (chunk_times, (function_name, _)) in functions_chunk_times.iter().zip(functions) {
        let mean_time: f64 = chunk_times.iter().sum::<f64>() / chunk_times.len() as f64;
        let standard_deviation: f64 = f64::sqrt(
            chunk_times
                .iter()
                .map(|time| (time - mean_time).powi(2))
                .sum::<f64>()
                / chunk_times.len() as f64,
        );

        println!(
            "{}: {:.0} iters per second ({:.1}ns±{:.1})",
            function_name,
            1.0 / mean_time,
            mean_time * 1_000_000_000.0,
            standard_deviation * 1_000_000_000.0,
        );
    }
}"#;

/// Benchmark small snippets of code
#[poise::command(
    prefix_command,
    broadcast_typing,
    track_edits,
    explanation_fn = "microbench_help"
)]
pub async fn microbench(
    ctx: Context<'_>,
    flags: poise::KeyValueArgs,
    code: poise::CodeBlock,
) -> Result<(), Error> {
    let user_code = &code.code;
    let black_box_hint = !user_code.contains("black_box");

    // insert convenience import for users
    let after_crate_attrs =
        "#![feature(bench_black_box)] #[allow(unused_imports)] use std::hint::black_box;\n";

    let pub_fn_indices = user_code.match_indices("pub fn ");
    if pub_fn_indices.clone().count() == 0 {
        ctx.say("No public functions (`pub fn`) found for benchmarking :thinking:")
            .await?;
        return Ok(());
    }

    // insert this after user code
    let mut after_code = BENCH_FUNCTION.to_owned();
    after_code += "fn main() {\nbench(&[";
    for (index, _) in pub_fn_indices {
        let function_name_start = index + "pub fn ".len();
        let function_name_end = match user_code[function_name_start..].find('(') {
            Some(x) => x + function_name_start,
            None => continue,
        };
        let function_name = user_code[function_name_start..function_name_end].trim();

        after_code += &format!("(\"{0}\", {0}), ", function_name);
    }
    after_code += "]);\n}\n";

    // final assembled code
    let code = hoise_crate_attributes(user_code, after_crate_attrs, &after_code);

    let (flags, mut flag_parse_errors) = parse_flags(flags);
    let mut result: PlayResult = ctx
        .data()
        .http
        .post("https://play.rust-lang.org/execute")
        .json(&PlaygroundRequest {
            code: &code,
            channel: Channel::Nightly, // has to be, for black_box
            crate_type: if code.contains("fn main") {
                CrateType::Binary
            } else {
                CrateType::Library
            },
            edition: flags.edition,
            mode: Mode::Release, // benchmarks on debug don't make sense
            tests: false,
        })
        .send()
        .await?
        .json()
        .await?;

    result.stderr = format_play_eval_stderr(&result.stderr, flags.warn);

    if black_box_hint {
        flag_parse_errors +=
            "Hint: use the black_box function to prevent computations from being optimized out\n";
    }
    send_reply(ctx, result, &code, &flags, &flag_parse_errors).await
}

pub fn microbench_help() -> String {
    generic_help(GenericHelp {
        command: "microbench",
        desc: "Benchmark small snippets of code by running them repeatedly. Public function \
        snippets are run in blocks of 10000 repetitions in a cycle until a certain time has \
        passed. Measurements are averaged and standard deviation is calculated for each",
        mode_and_channel: false,
        warn: true,
        example_code: "
pub fn snippet_a() { /* code */ }
pub fn snippet_b() { /* code */ }
",
    })
}
