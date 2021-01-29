//! run rust code on the rust-lang playground

use crate::{api, commands::Args, Error};

use reqwest::header;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Serialize)]
struct PlaygroundRequest<'a> {
    channel: Channel,
    edition: Edition,
    code: &'a str,
    #[serde(rename = "crateType")]
    crate_type: CrateType,
    mode: Mode,
    tests: bool,
}

impl<'a> PlaygroundRequest<'a> {
    fn new(code: &'a str) -> Self {
        PlaygroundRequest {
            channel: Channel::Nightly,
            edition: Edition::E2018,
            code,
            crate_type: CrateType::Binary,
            mode: Mode::Debug,
            tests: false,
        }
    }

    fn url_from_gist(&self, gist_id: &str) -> String {
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
            version, mode, edition, gist_id
        )
    }
}

#[derive(Debug, Serialize)]
struct MiriRequest<'a> {
    edition: Edition,
    code: &'a str,
}

impl MiriRequest<'_> {
    fn url_from_gist(&self, gist_id: &str) -> String {
        format!(
            "https://play.rust-lang.org/?edition={}&gist={}",
            match self.edition {
                Edition::E2015 => "2015",
                Edition::E2018 => "2018",
            },
            gist_id
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

    fn from_str(s: &str) -> Result<Self, Error> {
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

    fn from_str(s: &str) -> Result<Self, Error> {
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

    fn from_str(s: &str) -> Result<Self, Error> {
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

fn run_code_and_reply(args: &Args, code: &str) -> Result<(), Error> {
    let mut errors = String::new();

    let mut warnings = false;
    let mut request = PlaygroundRequest::new(code);

    match Channel::from_str(args.params.get("channel").unwrap_or(&"nightly")) {
        Ok(c) => request.channel = c,
        Err(e) => errors += &format!("{}\n", e),
    }

    match Mode::from_str(args.params.get("mode").unwrap_or(&"debug")) {
        Ok(m) => request.mode = m,
        Err(e) => errors += &format!("{}\n", e),
    }

    match Edition::from_str(args.params.get("edition").unwrap_or(&"2018")) {
        Ok(e) => request.edition = e,
        Err(e) => errors += &format!("{}\n", e),
    }

    match bool::from_str(args.params.get("warn").unwrap_or(&"false")) {
        Ok(e) => warnings = e,
        Err(_) => errors += "invalid warn bool\n",
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

    let result = if warnings {
        format!("{}\n{}", result.stderr, result.stdout)
    } else if result.success {
        result.stdout
    } else {
        result.stderr
    };

    if result.is_empty() {
        api::send_reply(&args, &format!("{}``` ```", errors))
    } else {
        crate::reply_potentially_long_text(
            &args,
            &format!("{}```\n{}", errors, result),
            "```",
            &format!(
                "Output too large. Playground link: {}",
                request.url_from_gist(&post_gist(&args, code)?),
            ),
        )
    }
}

/// Returns a gist ID
fn post_gist(args: &Args, code: &str) -> Result<String, Error> {
    let mut payload = HashMap::new();
    payload.insert("code", code);

    let resp = args
        .http
        .post("https://play.rust-lang.org/meta/gist/")
        .header(header::REFERER, "https://discord.gg/rust-lang")
        .json(&payload)
        .send()?;

    let mut resp: HashMap<String, String> = resp.json()?;
    info!("gist response: {:?}", resp);

    let gist_id = resp.remove("id").ok_or("no gist found")?;
    Ok(gist_id)
}

pub fn run(args: Args) -> Result<(), Error> {
    match crate::extract_code(args.body) {
        Some(code) => run_code_and_reply(&args, code),
        None => err(args),
    }
}

pub fn help(args: Args, name: &str) -> Result<(), Error> {
    let message = format!(
        "Compile and run rust code. All code is executed on https://play.rust-lang.org.
```?{} mode={{}} channel={{}} edition={{}} warn={{}} ``\u{200B}`code``\u{200B}` ```
Optional arguments:
    \tmode: debug, release (default: debug)
    \tchannel: stable, beta, nightly (default: nightly)
    \tedition: 2015, 2018 (default: 2018)
    \twarn: boolean flag to enable compilation warnings
    ",
        name
    );

    api::send_reply(&args, &message)?;
    Ok(())
}

pub fn miri_help(args: Args) -> Result<(), Error> {
    api::send_reply(
        &args,
        "Execute this program in the Miri interpreter to detect certain cases of undefined behavior
(like out-of-bounds memory access). All code is executed on https://play.rust-lang.org.
```?{} edition={{}} warn={{}} ``\u{200B}`code``\u{200B}` ```
Optional arguments:
    \tedition: 2015, 2018 (default: 2018)
    \twarn: boolean flag to enable compilation warnings",
    )?;
    Ok(())
}

pub fn eval(args: Args) -> Result<(), Error> {
    let code = match crate::extract_code(args.body) {
        Some(x) => x,
        None => return err(args),
    };

    if code.contains("fn main") {
        api::send_reply(&args, "code passed to ?eval should not contain `fn main`")?;
        return Ok(());
    }

    let mut full_code = String::from("fn main() {\n    println!(\"{:?}\", {\n");
    for line in code.lines() {
        full_code.push_str("        ");
        full_code.push_str(line);
        full_code.push_str("\n");
    }
    full_code.push_str("    });\n}");

    run_code_and_reply(&args, &full_code)
}

pub fn err(args: Args) -> Result<(), Error> {
    let message = "Missing code block. Please use the following markdown:
\\`code here\\`
or
\\`\\`\\`rust
code here
\\`\\`\\`";

    api::send_reply(&args, message)?;
    Ok(())
}

pub fn miri(args: Args) -> Result<(), Error> {
    let code = match crate::extract_code(args.body) {
        Some(x) => x,
        None => return err(args),
    };

    let mut errors = String::new();

    let mut warnings = false;
    let mut request = MiriRequest {
        code,
        edition: Edition::E2018,
    };

    match Edition::from_str(args.params.get("edition").unwrap_or(&"2018")) {
        Ok(e) => request.edition = e,
        Err(e) => errors += &format!("{}\n", e),
    }

    match bool::from_str(args.params.get("warn").unwrap_or(&"false")) {
        Ok(e) => warnings = e,
        Err(_) => errors += "invalid warn bool\n",
    }

    let resp = args
        .http
        .post("https://play.rust-lang.org/miri")
        .json(&request)
        .send()?;

    let result: PlayResult = resp.json()?;

    let result = if warnings {
        format!("{}\n{}", result.stderr, result.stdout)
    } else if result.success {
        result.stdout
    } else {
        result.stderr
    };

    if result.is_empty() {
        api::send_reply(&args, &format!("{}``` ```", errors))
    } else {
        crate::reply_potentially_long_text(
            &args,
            &format!("{}```\n{}", errors, result),
            "```",
            &format!(
                "Output too large. Playground link: {}",
                request.url_from_gist(&post_gist(&args, code)?),
            ),
        )
    }
}
