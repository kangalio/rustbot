//! run rust code on the rust-lang playground

use crate::{
    api,
    commands::{Args, Result},
};

use reqwest::header;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize)]
struct PlaygroundCode {
    channel: Channel,
    edition: Edition,
    code: String,
    #[serde(rename = "crateType")]
    crate_type: CrateType,
    mode: Mode,
    tests: bool,
}

impl PlaygroundCode {
    fn new(code: String) -> Self {
        PlaygroundCode {
            channel: Channel::Nightly,
            edition: Edition::E2018,
            code,
            crate_type: CrateType::Binary,
            mode: Mode::Debug,
            tests: false,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Channel {
    Stable,
    Beta,
    Nightly,
}

#[derive(Debug, Serialize)]
enum Edition {
    #[serde(rename = "2015")]
    E2015,
    #[serde(rename = "2018")]
    E2018,
}

#[derive(Debug, Serialize)]
enum CrateType {
    #[serde(rename = "bin")]
    Binary,
    #[serde(rename = "lib")]
    Library,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Debug,
    Release,
}

#[derive(Debug, Deserialize)]
struct PlayResult {
    success: bool,
    stdout: String,
    stderr: String,
}

fn run_code(args: &Args, code: &str) -> Result<String> {
    info!("sending request to playground.");
    let request = PlaygroundCode::new(code.to_string());

    let resp = args
        .http
        .post("https://play.rust-lang.org/execute")
        .json(&request)
        .send()?;

    let result: PlayResult = resp.json()?;

    let result = if result.success {
        result.stdout
    } else {
        result.stderr
    };

    Ok(if result.len() > 1994 {
        format!(
            "Output too large. Playground link: {}",
            get_playground_link(args, code)?
        )
    } else {
        format!("```{}```", result)
    })
}

fn get_playground_link(args: &Args, code: &str) -> Result<String> {
    let mut payload = HashMap::new();
    payload.insert("code", code);

    let resp = args
        .http
        .get("https://play.rust-lang.org/meta/gist/")
        .header(header::REFERER, "https://discord.gg/rust-lang")
        .json(&payload)
        .send()?;

    let resp = resp.text()?;
    debug!("{:?}", resp);

    Ok(resp)
}

pub fn run(args: Args) -> Result<()> {
    let code = args
        .params
        .get("code")
        .ok_or("Unable to retrieve param: query")?;

    let result = run_code(&args, code)?;
    api::send_reply(&args, &result)?;
    Ok(())
}

pub fn help(args: Args) -> Result<()> {
    let message = "Missing code block. Please use the following markdown:
\\`\\`\\`rust
    code here
\\`\\`\\`
    ";

    api::send_reply(&args, message)?;
    Ok(())
}

pub fn eval(args: Args) -> Result<()> {
    let code = args
        .params
        .get("code")
        .ok_or("Unable to retrieve param: query")?;

    let code = format!(
        "fn main(){{
    println!(\"{{:?}}\",{{ 
    {} 
    }});
}}",
        code
    );

    let result = run_code(&args, &code)?;
    api::send_reply(&args, &result)?;
    Ok(())
}

pub fn eval_help(args: Args) -> Result<()> {
    let message = "Missing code block. Please use the following markdown:
    \\`code here\\`
    or
    \\`\\`\\`rust
        code here
    \\`\\`\\`
    ";

    api::send_reply(&args, message)?;
    Ok(())
}
