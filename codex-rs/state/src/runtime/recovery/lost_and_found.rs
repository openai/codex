use super::quote_identifier;
use anyhow::Context;
use anyhow::Result;
use sqlx::Row;
use sqlx::SqlitePool;
use tracing::warn;

// If page 1 is destroyed, SQLite's recovery extension cannot discover the
// sqlite_schema root. It can still recover orphaned sqlite_schema rows into
// lost_and_found, so this module rebuilds tables from those rows and then
// copies matching lost_and_found record groups back into the recreated tables.

#[derive(Debug)]
struct SchemaObject {
    object_type: String,
    name: String,
    rootpage: i64,
    sql: String,
}

#[derive(Debug)]
struct TableColumn {
    cid: i64,
    name: String,
    column_type: String,
    pk: i64,
    hidden: i64,
}

pub(super) async fn rebuild_from_recovered_schema_if_needed(pool: &SqlitePool) -> Result<bool> {
    if !only_lost_and_found_table_exists(pool).await? {
        return Ok(false);
    }

    let schema = recovered_schema_objects(pool).await?;
    if schema
        .iter()
        .filter(|object| object.object_type == "table")
        .count()
        == 0
    {
        return Ok(false);
    }

    warn!(
        "SQLite recovery produced only lost_and_found rows; rebuilding tables from recovered schema rows"
    );
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(pool)
        .await?;

    for object in schema.iter().filter(|object| object.object_type == "table") {
        sqlx::query(object.sql.as_str())
            .execute(pool)
            .await
            .with_context(|| format!("failed to recreate recovered table {}", object.name))?;
    }

    let value_column_count = lost_and_found_value_column_count(pool).await?;
    for object in schema.iter().filter(|object| object.object_type == "table") {
        copy_lost_and_found_rows(pool, object, value_column_count).await?;
    }

    for object in schema.iter().filter(|object| object.object_type != "table") {
        if let Err(err) = sqlx::query(object.sql.as_str()).execute(pool).await {
            warn!(
                "skipping recovered {} {} during lost_and_found rebuild: {err}",
                object.object_type, object.name
            );
        }
    }

    Ok(true)
}

async fn only_lost_and_found_table_exists(pool: &SqlitePool) -> Result<bool> {
    let tables = sqlx::query_scalar::<_, String>(
        r#"
SELECT name
FROM sqlite_schema
WHERE type = 'table'
  AND name NOT LIKE 'sqlite_%'
  AND name != '_sqlx_migrations'
ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(tables.len() == 1 && tables[0] == "lost_and_found")
}

async fn recovered_schema_objects(pool: &SqlitePool) -> Result<Vec<SchemaObject>> {
    let rows = sqlx::query(
        r#"
SELECT
    CAST(c0 AS TEXT) AS object_type,
    CAST(c1 AS TEXT) AS name,
    CAST(c3 AS INTEGER) AS rootpage,
    CAST(c4 AS TEXT) AS sql
FROM lost_and_found
WHERE nfield = 5
  AND c0 IN ('table', 'index', 'trigger', 'view')
  AND typeof(c1) = 'text'
  AND typeof(c4) = 'text'
  AND c1 NOT LIKE 'sqlite_%'
ORDER BY
  CASE c0
    WHEN 'table' THEN 0
    WHEN 'view' THEN 1
    WHEN 'index' THEN 2
    WHEN 'trigger' THEN 3
    ELSE 4
  END,
  id
        "#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(SchemaObject {
                object_type: row.try_get("object_type")?,
                name: row.try_get("name")?,
                rootpage: row.try_get("rootpage")?,
                sql: row.try_get("sql")?,
            })
        })
        .collect()
}

async fn lost_and_found_value_column_count(pool: &SqlitePool) -> Result<i64> {
    let rows = sqlx::query("SELECT name FROM pragma_table_xinfo('lost_and_found')")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .iter()
        .filter_map(|row| row.try_get::<String, _>("name").ok())
        .filter_map(|name| name.strip_prefix('c')?.parse::<i64>().ok())
        .max()
        .map_or(0, |max_column| max_column + 1))
}

async fn copy_lost_and_found_rows(
    pool: &SqlitePool,
    table: &SchemaObject,
    value_column_count: i64,
) -> Result<()> {
    if table.rootpage <= 0 {
        return Ok(());
    }

    let columns = table_columns(pool, table.name.as_str()).await?;
    let mut column_names = Vec::new();
    let mut value_expressions = Vec::new();
    for column in columns.into_iter().filter(|column| column.hidden == 0) {
        column_names.push(quote_identifier(column.name.as_str()));
        if is_integer_primary_key(&column) {
            // INTEGER PRIMARY KEY values live in the b-tree rowid, not in the
            // record body, so sqlite_dbdata exposes them through lost_and_found.id.
            value_expressions.push("id".to_string());
        } else {
            if column.cid >= value_column_count {
                anyhow::bail!(
                    "lost_and_found table has {value_column_count} value columns, but recovered table {} needs c{}",
                    table.name,
                    column.cid
                );
            }
            value_expressions.push(format!("c{}", column.cid));
        }
    }

    if column_names.is_empty() {
        return Ok(());
    }

    let table_name = quote_identifier(table.name.as_str());
    let sql = format!(
        "INSERT OR REPLACE INTO main.{table_name} ({}) SELECT {} FROM lost_and_found WHERE rootpgno = ?",
        column_names.join(", "),
        value_expressions.join(", ")
    );
    sqlx::query(sql.as_str())
        .bind(table.rootpage)
        .execute(pool)
        .await
        .with_context(|| format!("failed to copy lost_and_found rows into {}", table.name))?;
    Ok(())
}

async fn table_columns(pool: &SqlitePool, table: &str) -> Result<Vec<TableColumn>> {
    let rows = sqlx::query(
        r#"
SELECT cid, name, type, pk, hidden
FROM pragma_table_xinfo(?)
ORDER BY cid
        "#,
    )
    .bind(table)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(TableColumn {
                cid: row.try_get("cid")?,
                name: row.try_get("name")?,
                column_type: row.try_get("type")?,
                pk: row.try_get("pk")?,
                hidden: row.try_get("hidden")?,
            })
        })
        .collect()
}

fn is_integer_primary_key(column: &TableColumn) -> bool {
    column.pk > 0 && column.column_type.eq_ignore_ascii_case("INTEGER")
}
