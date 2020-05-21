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
}

pub fn search(args: Args) -> Result<()> {
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

    if let Some(krate) = crate_list.crates.get(0) {
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

/// Print the help message
pub fn help(args: Args) -> Result<()> {
    let help_string = "search for a crate on crates.io
```
?crate query...
```";
    api::send_reply(&args, &help_string)?;
    Ok(())
}
