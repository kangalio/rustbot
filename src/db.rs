use diesel::prelude::*;
use std::result::Result as StdResult;

pub(crate) fn database_connection() -> StdResult<SqliteConnection, Box<std::error::Error>> {
    Ok(SqliteConnection::establish(&std::env::var(
        "DATABASE_URL",
    )?)?)
}
