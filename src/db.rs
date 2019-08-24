use crate::commands::Result;
use diesel::prelude::*;

pub(crate) fn database_connection() -> Result<PgConnection> {
    Ok(PgConnection::establish(&std::env::var("DATABASE_URL")?)?)
}

pub(crate) fn run_migrations() -> Result<()> {
    let migrations_dir = std::env::var("MIGRATIONS_DIR")
        .map(|p| std::path::PathBuf::from(p))
        .unwrap_or_else(|_| std::path::PathBuf::from("migrations"));

    diesel_migrations::run_pending_migrations_in_directory(
        &database_connection()?,
        &migrations_dir,
        &mut std::io::sink(),
    )?;

    Ok(())
}
