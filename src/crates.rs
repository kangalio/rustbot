use crate::{api, Context, Error};

use reqwest::header;
use serde::Deserialize;
use serenity::model::prelude::*;
use serenity_framework::prelude::*;

const USER_AGENT: &str = "github.com/kangalioo/rustbot";

#[derive(Debug, Deserialize)]
struct Crates {
    crates: Vec<Crate>,
}
#[derive(Debug, Deserialize)]
struct Crate {
    id: String,
    name: String,
    newest_version: String,
    updated_at: String,
    downloads: u64,
    description: Option<String>,
    documentation: Option<String>,
    exact_match: bool,
}

/// Queries the crates.io crates list and yields the first result, if any
async fn get_crate(http: &reqwest::Client, query: &str) -> Result<Option<Crate>, Error> {
    info!("searching for crate `{}`", query);

    let crate_list = http
        .get("https://crates.io/api/v1/crates")
        .header(header::USER_AGENT, USER_AGENT)
        .query(&[("q", query)])
        .send()
        .await?
        .json::<Crates>()
        .await?;

    Ok(crate_list.crates.into_iter().next())
}

#[command]
/// Search for a crate on crates.io
// TODO: somehow figure out how to rename the user-facing command name to "crate" (no underscore)
pub async fn crate_(ctx: Context, msg: &Message, crate_name: String) -> Result<(), Error> {
    if let Some(url) = rustc_crate_link(&crate_name) {
        return api::send_reply(&ctx, msg, url).await;
    }

    match get_crate(&ctx.data.reqwest, &crate_name).await? {
        Some(crate_result) => {
            if crate_result.exact_match {
                msg.channel_id
                    .send_message(&ctx.serenity_ctx.http, |m| {
                        m.embed(|e| {
                            e.title(&crate_result.name)
                                .url(format!("https://crates.io/crates/{}", crate_result.id))
                                .description(
                                    &crate_result
                                        .description
                                        .as_deref()
                                        .unwrap_or("_<no description available>_"),
                                )
                                .field("Version", &crate_result.newest_version, true)
                                .field("Downloads", &crate_result.downloads, true)
                                .timestamp(crate_result.updated_at.as_str())
                        })
                    })
                    .await?;
            } else {
                api::send_reply(
                    &ctx,
                    msg,
                    &format!(
                        "Crate `{}` not found. Did you mean `{}`?",
                        crate_name, crate_result.name
                    ),
                )
                .await?;
            }
        }
        None => api::send_reply(&ctx, msg, &format!("Crate `{}` not found", crate_name)).await?,
    };
    Ok(())
}

/// Provide the documentation link to an official Rust crate (e.g. std, alloc, nightly)
fn rustc_crate_link(crate_name: &str) -> Option<&'static str> {
    match crate_name.to_ascii_lowercase().as_str() {
        "std" => Some("https://doc.rust-lang.org/stable/std/"),
        "core" => Some("https://doc.rust-lang.org/stable/core/"),
        "alloc" => Some("https://doc.rust-lang.org/stable/alloc/"),
        "proc_macro" => Some("https://doc.rust-lang.org/stable/proc_macro/"),
        "beta" => Some("https://doc.rust-lang.org/beta/std/"),
        "nightly" => Some("https://doc.rust-lang.org/nightly/std/"),
        "rustc" => Some("https://doc.rust-lang.org/nightly/nightly-rustc/"),
        "test" => Some("https://doc.rust-lang.org/stable/test"),
        _ => None,
    }
}

#[command]
/// Retrieve documentation for a given crate
pub async fn docs(ctx: Context, msg: &Message, query: String) -> Result<(), Error> {
    let mut query_iter = query.splitn(2, "::");
    let crate_name = query_iter.next().unwrap();

    // The base docs url, e.g. `https://docs.rs/syn` or `https://doc.rust-lang.org/stable/std/`
    let mut doc_url = if let Some(rustc_crate) = rustc_crate_link(crate_name) {
        rustc_crate.to_string()
    } else {
        let crate_ = match get_crate(&ctx.data.reqwest, crate_name).await? {
            Some(x) => x,
            None => {
                return api::send_reply(&ctx, msg, &format!("Crate `{}` not found", crate_name))
                    .await
            }
        };

        let crate_name = crate_.name;
        crate_
            .documentation
            .unwrap_or_else(|| format!("https://docs.rs/{}", crate_name))
    };

    if let Some(item_path) = query_iter.next() {
        doc_url += "?search=";
        doc_url += item_path;
    }

    api::send_reply(&ctx, msg, &doc_url).await
}
