use super::GodboltMode;
use crate::{Data, Error, PrefixContext};
use chrono::TimeZone as _;
use sqlx::Connection as _;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GodboltTarget {
    id: String,
    name: String,
    lang: String,
    compiler_type: String,
    semver: String,
    instruction_set: String,
}

impl GodboltTarget {
    fn clean_request_data(&mut self) {
        // Some semvers get weird characters like `()` in them or spaces, we strip that out here
        self.semver = self
            .semver
            .chars()
            .filter(|char| char.is_alphanumeric() || matches!(char, '.' | '-' | '_'))
            .map(|char| char.to_ascii_lowercase())
            .collect();
    }
}

async fn update_godbolt_targets(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    data: &Data,
) -> Result<(), Error> {
    // Fetch the last time we updated the targets list, this will be null/none if we've never
    // done it before
    let last_update_time = sqlx::query!("SELECT last_update FROM last_godbolt_update")
        .fetch_optional(&mut *conn)
        .await?;

    let needs_update = if let Some(last_update_time) = last_update_time {
        // Convert the stored timestamp into a utc date time
        let last_update_time = chrono::Utc.timestamp_nanos(last_update_time.last_update);

        // Get the time to wait between each update of the godbolt targets list
        let update_period = chrono::Duration::from_std(
            std::env::var("GODBOLT_UPDATE_DURATION")
                .ok()
                .and_then(|duration| duration.parse::<u64>().ok())
                .map(std::time::Duration::from_secs)
                // Currently set to 12 hours
                .unwrap_or_else(|| std::time::Duration::from_secs(60 * 60 * 12)),
        )?;

        let time_since_update = chrono::Utc::now().signed_duration_since(last_update_time);
        let needs_update = time_since_update >= update_period;
        if needs_update {
            log::info!(
                "godbolt targets were last updated {:#?} ago, updating them",
                time_since_update,
            );
        }

        needs_update
    } else {
        log::info!("godbolt targets haven't yet been updated, fetching them");

        true
    };

    // If we should perform an update then do so
    if needs_update {
        let request = data
            .http
            .get("https://godbolt.org/api/compilers/rust")
            .header(reqwest::header::ACCEPT, "application/json")
            .build()?;

        let mut targets: Vec<GodboltTarget> = data.http.execute(request).await?.json().await?;
        log::info!("got {} godbolt targets", targets.len());

        // Clean up the data we've gotten from the request
        for target in &mut targets {
            target.clean_request_data();
        }

        // Run the target updates within a transaction so that if things go wrong we don't
        // end up with an empty targets list
        conn.transaction::<_, (), Error>(|conn| {
            Box::pin(async move {
                // Remove all old values from the targets list, this is probably overly-cautious
                // but it at least ensures that we never have incorrect targets within the db
                sqlx::query!("DELETE FROM godbolt_targets")
                    .execute(&mut *conn)
                    .await?;

                // Insert all of our newly fetched targets into the db
                for target in targets {
                    // Some versions have a leading `rustc ` (`rustc beta`, `rustc 1.40.0`, etc.),
                    // so we strip that here so we only have to do it once
                    let semver = target
                        .semver
                        .strip_prefix("rustc ")
                        .unwrap_or(&*target.semver);

                    sqlx::query!(
                        "INSERT INTO godbolt_targets (
                            id,
                            name,
                            lang,
                            compiler_type,
                            semver,
                            instruction_set
                         )
                         VALUES ($1, $2, $3, $4, $5, $6)",
                        target.id,
                        target.name,
                        target.lang,
                        target.compiler_type,
                        semver,
                        target.instruction_set,
                    )
                    .execute(&mut *conn)
                    .await?;
                }

                // Get the current utc timestamp
                let current_time = chrono::Utc::now().timestamp_nanos();

                // Set the last godbolt update time to now
                sqlx::query!(
                    "INSERT INTO last_godbolt_update (id, last_update)
                     VALUES (0, $1)
                     ON CONFLICT(id) DO UPDATE SET last_update = $1
                     WHERE id = 0",
                    current_time,
                )
                .execute(&mut *conn)
                .await?;

                Ok(())
            })
        })
        .await?;

        log::info!("finished updating godbolt targets list");
    }

    Ok(())
}

async fn fetch_godbolt_targets(data: &Data) -> Result<Vec<GodboltTarget>, Error> {
    let mut conn = data.database.acquire().await?;

    // If we encounter an error while updating the targets list, just log it
    if let Err(error) = update_godbolt_targets(&mut conn, data).await {
        log::error!("failed to update godbolt targets list: {:?}", error);
    }

    log::info!("fetching godbolt targets");
    let query = sqlx::query!(
        "SELECT id, name, lang, compiler_type, semver, instruction_set FROM godbolt_targets"
    )
    .fetch_all(&mut conn)
    .await?;

    let targets = query
        .into_iter()
        .map(|target| GodboltTarget {
            id: target.id,
            name: target.name,
            lang: target.lang,
            compiler_type: target.compiler_type,
            semver: target.semver,
            instruction_set: target.instruction_set,
        })
        .collect::<Vec<_>>();

    log::debug!("fetched {} godbolt targets", targets.len());
    Ok(targets)
}

// Generates godbolt-compatible rustc identifier and flags from command input
//
// Transforms human readable rustc version (e.g. "1.34.1") into compiler id on godbolt (e.g. "r1341")
// Full list of version<->id can be obtained at https://godbolt.org/api/compilers/rust
pub(super) async fn rustc_id_and_flags(
    data: &Data,
    params: &poise::KeyValueArgs,
    mode: GodboltMode,
) -> Result<(String, String), Error> {
    let rustc = params.get("rustc").unwrap_or("nightly");
    let targets = fetch_godbolt_targets(data).await?;
    let target = targets.into_iter().find(|target| target.semver == rustc.trim()).ok_or(
        "the `rustc` argument should be a version specifier like `nightly` `beta` or `1.45.2`. \
        Run ?targets for a full list",
    )?;

    let mut flags = params
        .get("flags")
        .unwrap_or("-Copt-level=3 --edition=2018")
        .to_owned();
    if mode == GodboltMode::LlvmIr {
        flags += " --emit=llvm-ir -Cdebuginfo=0";
    }

    Ok((target.id, flags))
}

/// Used to rank godbolt compiler versions for listing them out
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SemverRanking<'a> {
    Beta,
    Nightly,
    Compiler(&'a str),
    Semver(std::cmp::Reverse<(u16, u16, u16)>),
}

impl<'a> From<&'a str> for SemverRanking<'a> {
    fn from(semver: &'a str) -> Self {
        match semver {
            "beta" => Self::Beta,
            "nightly" => Self::Nightly,

            semver => {
                // Rustc versions are received in a `X.X.X` form, so we parse out
                // the major/minor/patch versions and then order them in *reverse*
                // order based on their version triple, this means that the most
                // recent (read: higher) versions will be at the top of the list
                let mut version_triple = semver.splitn(3, '.');
                let version_triple = version_triple
                    .next()
                    .zip(version_triple.next())
                    .zip(version_triple.next())
                    .and_then(|((major, minor), patch)| {
                        Some((
                            major.parse().ok()?,
                            minor.parse().ok()?,
                            patch.parse().ok()?,
                        ))
                    });

                // If we successfully parsed out a semver tuple, return it
                if let Some((major, minor, patch)) = version_triple {
                    Self::Semver(std::cmp::Reverse((major, minor, patch)))

                // Anything that doesn't fit the `X.X.X` format we treat as an alternative
                // compiler, we list these after beta & nightly but before the many canonical
                // rustc versions
                } else {
                    Self::Compiler(semver)
                }
            }
        }
    }
}

/// Lists all available godbolt rustc targets
#[poise::command(prefix_command, broadcast_typing)]
pub async fn targets(ctx: PrefixContext<'_>) -> Result<(), Error> {
    let mut targets = fetch_godbolt_targets(&ctx.data).await?;

    // Can't use sort_by_key because https://github.com/rust-lang/rust/issues/34162
    targets.sort_unstable_by(|lhs, rhs| {
        SemverRanking::from(&*lhs.semver).cmp(&SemverRanking::from(&*rhs.semver))
    });

    poise::send_reply(ctx.into(), |msg| {
        msg.embed(|embed| {
            embed
                .title("Godbolt Targets")
                .fields(targets.into_iter().map(|target| {
                    (
                        target.semver,
                        format!("{} (runs on {})", target.name, target.instruction_set),
                        true,
                    )
                }))
        })
    })
    .await?;

    Ok(())
}
