use crate::commands::Result;
use diesel::prelude::*;
use diesel::r2d2;
use lazy_static::lazy_static;

type Pool = r2d2::Pool<r2d2::ConnectionManager<PgConnection>>;

lazy_static! {
    pub(crate) static ref DB: Pool = Pool::new(r2d2::ConnectionManager::<PgConnection>::new(
        &std::env::var("DATABASE_URL").expect("DATABASE_URL not set")
    ))
    .expect("Unable to connect to database");
}

pub(crate) fn run_migrations() -> Result<()> {
    let conn = PgConnection::establish(&std::env::var("DATABASE_URL")?)?;

    diesel_migrations::embed_migrations!();

    info!("Running database migrations");
    let _ = embedded_migrations::run_with_output(&conn, &mut std::io::stdout())?;

    Ok(())
}
