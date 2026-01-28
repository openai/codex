use sqlx::migrate::Migrator;

pub(crate) static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
