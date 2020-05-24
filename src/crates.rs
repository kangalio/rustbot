use crate::{
    api,
    commands::{Args, Result},
};

use reqwest::header;
use serde::Deserialize;

const USER_AGENT: &str = "rust-lang/discord-mods-bot";

#[derive(Debug, Deserialize)]
struct Crates {
    crates: Vec<Crate>,
}
#[derive(Debug, Deserialize)]
struct Crate {
    id: String,
    name: String,
    #[serde(rename = "newest_version")]
    version: String,
    #[serde(rename = "updated_at")]
    updated: String,
    downloads: u64,
    description: String,
    documentation: Option<String>,
}

fn get_crate(args: &Args) -> Result<Option<Crate>> {
    let query = args
        .params
        .get("query")
        .ok_or("Unable to retrieve param: query")?;

    info!("searching for crate `{}`", query);

    let crate_list = args
        .http
        .get("https://crates.io/api/v1/crates")
        .header(header::USER_AGENT, USER_AGENT)
        .query(&[("q", query)])
        .send()?
        .json::<Crates>()?;

    Ok(crate_list.crates.into_iter().nth(0))
}

pub fn search(args: Args) -> Result<()> {
    if let Some(krate) = get_crate(&args)? {
        args.msg.channel_id.send_message(&args.cx, |m| {
            m.embed(|e| {
                e.title(&krate.name)
                    .url(format!("https://crates.io/crates/{}", krate.id))
                    .description(&krate.description)
                    .field("version", &krate.version, true)
                    .field("downloads", &krate.downloads, true)
                    .timestamp(krate.updated.as_str())
            });

            m
        })?;
    } else {
        let message = "No crates found.";
        api::send_reply(&args, message)?;
    }

    Ok(())
}

pub fn doc_search(args: Args) -> Result<()> {
    if let Some(krate) = get_crate(&args)? {
        let name = krate.name;
        let message = krate
            .documentation
            .unwrap_or_else(|| format!("https://docs.rs/{}", name));

        api::send_reply(&args, &message)?;
    } else {
        let message = "No crates found.";
        api::send_reply(&args, message)?;
    }

    Ok(())
}

/// Print the help message
pub fn help(args: Args) -> Result<()> {
    let help_string = "search for a crate on crates.io
```
?crate query...
```";
    api::send_reply(&args, &help_string)?;
    Ok(())
}

/// Print the help message
pub fn doc_help(args: Args) -> Result<()> {
    let help_string = "retrieves documentation for a given crate
```
?docs query...
```";
    api::send_reply(&args, &help_string)?;
    Ok(())
}
