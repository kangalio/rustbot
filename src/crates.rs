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
    name: String,
    // newest_version: String, // https://github.com/kangalioo/rustbot/issues/23
    max_version: Option<String>,
    max_stable_version: Option<String>, // sometimes null empirically
    updated_at: String,
    downloads: u64,
    description: Option<String>,
    documentation: Option<String>,
    exact_match: bool,
}

/// Queries the crates.io crates list for a specific crate
async fn get_crate(http: &reqwest::Client, query: &str) -> Result<Crate, Error> {
    log::info!("searching for crate `{}`", query);

    let crate_list = http
        .get("https://crates.io/api/v1/crates")
        .header(header::USER_AGENT, USER_AGENT)
        .query(&[("q", query)])
        .send()
        .await?
        .json::<Crates>()
        .await
        .map_err(|e| format!("Cannot parse crates.io JSON response (`{}`)", e))?;

    let crate_ = crate_list
        .crates
        .into_iter()
        .next()
        .ok_or_else(|| format!("Crate `{}` not found", query))?;

    if crate_.exact_match {
        Ok(crate_)
    } else {
        Err(format!(
            "Crate `{}` not found. Did you mean `{}`?",
            query, crate_.name
        )
        .into())
    }
}

fn get_documentation(crate_: &Crate) -> String {
    match &crate_.documentation {
        Some(doc) => doc.to_owned(),
        None => format!("https://docs.rs/{}", crate_.name),
    }
}

/// 6051423 -> "6 051 423"
fn format_number(mut n: u64) -> String {
    let mut output = String::new();
    while n >= 1000 {
        output.insert_str(0, &format!(" {:03}", n % 1000));
        n /= 1000;
    }
    output.insert_str(0, &format!("{}", n));
    output
}

async fn autocomplete_crate(ctx: Context<'_>, partial: String) -> impl Iterator<Item = String> {
    let http = &ctx.data().http;

    let response = http
        .get("https://crates.io/api/v1/crates")
        .header(header::USER_AGENT, USER_AGENT)
        .query(&[("q", &*partial), ("per_page", "25"), ("sort", "downloads")])
        .send()
        .await;

    let crate_list = match response {
        Ok(response) => response.json::<Crates>().await.ok(),
        Err(_) => None,
    };

    crate_list
        .map_or(Vec::new(), |list| list.crates)
        .into_iter()
        .map(|crate_| crate_.name)
}

/// Lookup crates on crates.io
///
/// Search for a crate on crates.io
/// ```
/// ?crate crate_name
/// ```
#[poise::command(
    prefix_command,
    rename = "crate",
    broadcast_typing,
    track_edits,
    slash_command,
    category = "Crates"
)]
pub async fn crate_(
    ctx: Context<'_>,
    #[description = "Name of the searched crate"]
    #[autocomplete = "autocomplete_crate"]
    crate_name: String,
) -> Result<(), Error> {
    if let Some(url) = rustc_crate_link(&crate_name) {
        ctx.say(url).await?;
        return Ok(());
    }

    let crate_ = get_crate(&ctx.data().http, &crate_name).await?;
    ctx.send(|m| {
        m.embed(|e| {
            e.title(&crate_.name)
                .url(get_documentation(&crate_))
                .description(
                    &crate_
                        .description
                        .as_deref()
                        .unwrap_or("_<no description available>_"),
                )
                .field(
                    "Version",
                    crate_
                        .max_stable_version
                        .or(crate_.max_version)
                        .unwrap_or_else(|| "<unknown version>".into()),
                    true,
                )
                .field("Downloads", format_number(crate_.downloads), true)
                .timestamp(crate_.updated_at.as_str())
                .color(crate::EMBED_COLOR)
        })
    })
    .await?;

    Ok(())
}

/// Returns whether the given type name is the one of a primitive.
fn is_primitive(name: &str) -> bool {
    matches!(
        name,
        "f32"
            | "f64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "pointer"
            | "reference"
            | "array"
            | "bool"
            | "char"
            | "fn"
            | "slice"
            | "str"
            | "tuple"
            | "unit"
    )
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
#[poise::command(
    prefix_command,
    aliases("docs"),
    broadcast_typing,
    track_edits,
    slash_command,
    category = "Crates"
)]
pub async fn doc(
    ctx: Context<'_>,
    #[description = "Path of the crate and item to lookup"] query: String,
) -> Result<(), Error> {
    let mut query_iter = query.splitn(2, "::");
    let crate_name = query_iter.next().unwrap();

    let mut doc_url = if let Some(rustc_crate) = rustc_crate_link(crate_name) {
        rustc_crate.to_owned()
    } else if crate_name.is_empty() || is_primitive(crate_name) {
        "https://doc.rust-lang.org/stable/std/".to_owned()
    } else {
        get_documentation(&get_crate(&ctx.data().http, crate_name).await?)
    };

    if is_primitive(crate_name) {
        doc_url += "?search=";
        doc_url += &query;
    } else {
        if let Some(item_path) = query_iter.next() {
            doc_url += "?search=";
            doc_url += item_path;
        }
    }

    ctx.say(doc_url).await?;

    Ok(())
}
