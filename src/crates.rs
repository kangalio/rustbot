use crate::{Context, Error};

use reqwest::header;
use serde::Deserialize;

const USER_AGENT: &str = "kangalioo/rustbot";

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
    log::info!("searching for crate `{}`", query);

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

/// Lookup crates on crates.io
///
/// Search for a crate on crates.io
/// ```
/// ?crate crate_name
/// ```
#[poise::command(rename = "crate", broadcast_typing, track_edits)]
pub async fn crate_(ctx: Context<'_>, crate_name: String) -> Result<(), Error> {
    if let Some(url) = rustc_crate_link(&crate_name) {
        poise::say_reply(ctx, url.to_owned()).await?;
        return Ok(());
    }

    match get_crate(&ctx.data.http, &crate_name).await? {
        Some(found_crate) => {
            if found_crate.exact_match {
                poise::send_reply(ctx, |m| {
                    m.embed(|e| {
                        e.title(&found_crate.name)
                            .url(format!("https://crates.io/crates/{}", found_crate.id))
                            .description(
                                &found_crate
                                    .description
                                    .as_deref()
                                    .unwrap_or("_<no description available>_"),
                            )
                            .field("Version", &found_crate.newest_version, true)
                            .field("Downloads", &found_crate.downloads, true)
                            .timestamp(found_crate.updated_at.as_str())
                    })
                })
                .await?;
            } else {
                poise::say_reply(
                    ctx,
                    format!(
                        "Crate `{}` not found. Did you mean `{}`?",
                        crate_name, found_crate.name
                    ),
                )
                .await?;
            }
        }
        None => poise::say_reply(ctx, format!("Crate `{}` not found", crate_name)).await?,
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
/// Lookup documentation
///
/// Retrieve documentation for a given crate
/// ```
/// ?docs crate_name::module::item
/// ```
#[poise::command(aliases("docs"), broadcast_typing, track_edits)]
pub async fn doc(ctx: Context<'_>, query: String) -> Result<(), Error> {
    let mut query_iter = query.splitn(2, "::");
    let crate_name = query_iter.next().unwrap();

    // The base docs url, e.g. `https://docs.rs/syn` or `https://doc.rust-lang.org/stable/std/`
    let mut doc_url = if let Some(rustc_crate) = rustc_crate_link(crate_name) {
        rustc_crate.to_owned()
    } else if crate_name.is_empty() {
        "https://doc.rust-lang.org/stable/std/".to_owned()
    } else {
        let crate_ = match get_crate(&ctx.data.http, crate_name).await? {
            Some(x) => x,
            None => {
                poise::say_reply(ctx, format!("Crate `{}` not found", crate_name)).await?;
                return Ok(());
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

    poise::say_reply(ctx, doc_url).await?;

    Ok(())
}
