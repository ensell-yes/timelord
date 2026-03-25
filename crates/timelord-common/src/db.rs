use sqlx::postgres::{PgPool, PgPoolOptions};

/// Create a connection pool for PostgreSQL.
pub async fn create_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Run migrations from a directory path.
/// Uses `set_ignore_missing(true)` so services tolerate migrations owned by other services.
pub async fn run_migrations(pool: &PgPool, migrations_path: &str) -> anyhow::Result<()> {
    let mut migrator = sqlx::migrate::Migrator::new(std::path::Path::new(migrations_path)).await?;
    migrator.set_ignore_missing(true);
    migrator.run(pool).await?;
    tracing::info!(path = migrations_path, "migrations applied");
    Ok(())
}

/// Set the RLS tenant context for the current transaction.
/// Must be called inside a transaction before any tenant-scoped queries.
pub async fn set_rls_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    org_id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT set_config('app.current_org_id', $1, true)")
        .bind(org_id.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}
