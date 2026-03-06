use sqlx::migrate::Migrator;

pub(crate) static STATE_MIGRATOR: Migrator = sqlx::migrate!("./migrations");
pub(crate) static LOG_MIGRATOR: Migrator = sqlx::migrate!("./log_migrations");
