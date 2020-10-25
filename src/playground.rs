//! run rust code on the rust-lang playground

use crate::{
    api,
    commands::{Args, Result},
};

use reqwest::header;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Serialize)]
struct PlaygroundCode<'a> {
    channel: Channel,
    edition: Edition,
    code: &'a str,
    #[serde(rename = "crateType")]
    crate_type: CrateType,
    mode: Mode,
    tests: bool,
}

impl<'a> PlaygroundCode<'a> {
    fn new(code: &'a str) -> Self {
        PlaygroundCode {
            channel: Channel::Nightly,
            edition: Edition::E2018,
            code,
            crate_type: CrateType::Binary,
            mode: Mode::Debug,
            tests: false,
        }
    }

    fn url_from_gist(&self, gist: &str) -> String {
        let version = match self.channel {
            Channel::Nightly => "nightly",
            Channel::Beta => "beta",
            Channel::Stable => "stable",
        };

        let edition = match self.edition {
            Edition::E2015 => "2015",
            Edition::E2018 => "2018",
        };

        let mode = match self.mode {
            Mode::Debug => "debug",
            Mode::Release => "release",
        };

        format!(
            "https://play.rust-lang.org/?version={}&mode={}&edition={}&gist={}",
            version, mode, edition, gist
        )
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Channel {
    Stable,
    Beta,
    Nightly,
}

impl FromStr for Channel {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "stable" => Ok(Channel::Stable),
            "beta" => Ok(Channel::Beta),
            "nightly" => Ok(Channel::Nightly),
            _ => Err(format!("invalid release channel `{}`", s).into()),
        }
    }
}

#[derive(Debug, Serialize)]
enum Edition {
    #[serde(rename = "2015")]
    E2015,
    #[serde(rename = "2018")]
    E2018,
}

impl FromStr for Edition {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "2015" => Ok(Edition::E2015),
            "2018" => Ok(Edition::E2018),
            _ => Err(format!("invalid edition `{}`", s).into()),
        }
    }
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

impl FromStr for Mode {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "debug" => Ok(Mode::Debug),
            "release" => Ok(Mode::Release),
            _ => Err(format!("invalid compilation mode `{}`", s).into()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct PlayResult {
    success: bool,
    stdout: String,
    stderr: String,
}

fn run_code(args: &Args, code: &str) -> Result<String> {
    let mut errors = String::new();

    let channel = args.params.get("channel").unwrap_or_else(|| &"nightly");
    let mode = args.params.get("mode").unwrap_or_else(|| &"debug");
    let edition = args.params.get("edition").unwrap_or_else(|| &"2018");

    let mut request = PlaygroundCode::new(code);

    match Channel::from_str(channel) {
        Ok(c) => request.channel = c,
        Err(e) => errors += &format!("{}\n", e),
    }

    match Mode::from_str(mode) {
        Ok(m) => request.mode = m,
        Err(e) => errors += &format!("{}\n", e),
    }

    match Edition::from_str(edition) {
        Ok(e) => request.edition = e,
        Err(e) => errors += &format!("{}\n", e),
    }

    if !code.contains("fn main") {
        request.crate_type = CrateType::Library;
    }

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

    Ok(if result.len() + errors.len() > 1994 {
        format!(
            "{}Output too large. Playground link: {}",
            errors,
            get_playground_link(args, code, &request)?
        )
    } else if result.len() == 0 {
        format!("{}compilation succeded.", errors)
    } else {
        format!("{}```{}```", errors, result)
    })
}

fn get_playground_link(args: &Args, code: &str, request: &PlaygroundCode) -> Result<String> {
    let mut payload = HashMap::new();
    payload.insert("code", code);

    let resp = args
        .http
        .post("https://play.rust-lang.org/meta/gist/")
        .header(header::REFERER, "https://discord.gg/rust-lang")
        .json(&payload)
        .send()?;

    let resp: HashMap<String, String> = resp.json()?;
    info!("gist response: {:?}", resp);

    resp.get("id")
        .map(|id| request.url_from_gist(id))
        .ok_or_else(|| "no gist found".into())
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

pub fn help(args: Args, name: &str) -> Result<()> {
    let message = format!(
        "Compile and run rust code. All code is executed on https://play.rust-lang.org.
```?{} mode={{}} channel={{}} edition={{}} ``\u{200B}`code``\u{200B}` ```
Optional arguments:
    \tmode: debug, release (default: debug)
    \tchannel: stable, beta, nightly (default: nightly)
    \tedition: 2015, 2018 (default: 2018)
    ",
        name
    );

    api::send_reply(&args, &message)?;
    Ok(())
}

pub fn err(args: Args) -> Result<()> {
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

pub fn eval_err(args: Args) -> Result<()> {
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
