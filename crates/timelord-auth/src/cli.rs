use clap::{Parser, Subcommand};
use dotenvy::dotenv;

// Re-use auth crate modules
use timelord_auth::{
    models::org_member::OrgRole,
    repo::{org_repo, user_repo},
    services::password,
};
use timelord_common::{audit::{insert_audit, AuditEntry}, db};

#[derive(Parser)]
#[command(name = "timelord-cli", about = "Timelord administration CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create the first system admin user
    CreateAdmin {
        /// Admin email address
        #[arg(long)]
        email: String,
        /// Admin password
        #[arg(long)]
        password: String,
        /// Display name (defaults to email)
        #[arg(long)]
        name: Option<String>,
        /// Force creation even if an admin already exists
        #[arg(long)]
        force: bool,
    },
    /// Reset a local user's password
    ResetPassword {
        /// User email address
        #[arg(long)]
        email: String,
        /// New password
        #[arg(long)]
        password: String,
    },
    /// Check database connectivity and migration status
    DbCheck,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    timelord_common::telemetry::init("timelord-cli");

    let cli = Cli::parse();
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL not set. Run `timelord-cli init` first or set it in .env"))?;

    let pool = db::create_pool(&database_url).await?;
    db::run_migrations(&pool, "crates/timelord-auth/migrations").await?;

    match cli.command {
        Commands::CreateAdmin { email, password, name, force } => {
            create_admin(&pool, &email, &password, name.as_deref(), force).await?;
        }
        Commands::ResetPassword { email, password } => {
            reset_password(&pool, &email, &password).await?;
        }
        Commands::DbCheck => {
            db_check(&pool).await?;
        }
    }

    Ok(())
}

async fn create_admin(
    pool: &sqlx::PgPool,
    email: &str,
    password: &str,
    name: Option<&str>,
    force: bool,
) -> anyhow::Result<()> {
    let email = email.trim().to_lowercase();

    // Check if admin already exists
    let has_admin = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM users WHERE system_admin = true) AS "exists!""#
    )
    .fetch_one(pool)
    .await?;

    if has_admin && !force {
        anyhow::bail!("A system admin already exists. Use --force to create another.");
    }

    let display_name = name.unwrap_or(&email);
    let hash = password::hash_password(password)?;

    let mut tx = pool.begin().await?;

    let user = user_repo::create_local_user(&mut *tx, &email, display_name, &hash, true).await?;
    println!("Created admin user: {} ({})", user.email, user.id);

    // Create personal org
    let slug = format!("personal-{}", &user.id.to_string()[..8]);
    let org = org_repo::create(&mut *tx, "Personal", &slug, true).await?;
    println!("Created personal org: {} ({})", org.name, org.id);

    // Set RLS context before org_members insert
    db::set_rls_context(&mut tx, org.id).await?;

    org_repo::add_member(&mut *tx, org.id, user.id, OrgRole::Owner).await?;
    user_repo::update_last_active_org(&mut *tx, user.id, org.id).await?;

    insert_audit(
        &mut *tx,
        AuditEntry::new(org.id, "cli_create_admin", "user").entity(user.id),
    )
    .await;

    // Mark setup as complete
    sqlx::query!(
        r#"
        INSERT INTO system_settings (key, value) VALUES ('setup_complete', 'true'::jsonb)
        ON CONFLICT (key) DO UPDATE SET value = 'true'::jsonb, updated_at = now()
        "#
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    println!("Setup complete. You can now start timelord-auth and log in.");
    Ok(())
}

async fn reset_password(pool: &sqlx::PgPool, email: &str, password: &str) -> anyhow::Result<()> {
    let email = email.trim().to_lowercase();
    let user = user_repo::find_local_by_email(pool, &email)
        .await?
        .ok_or_else(|| anyhow::anyhow!("No local user found with email: {email}"))?;

    let hash = password::hash_password(password)?;
    user_repo::update_password(pool, user.id, &hash).await?;

    println!("Password reset for user: {} ({})", user.email, user.id);
    Ok(())
}

async fn db_check(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    // Test connectivity
    let row = sqlx::query_scalar!("SELECT 1 AS \"one!\"")
        .fetch_one(pool)
        .await?;
    println!("Database connection: OK (result={})", row);

    // Check migration status
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*)::bigint AS "count!" FROM _sqlx_migrations"#
    )
    .fetch_one(pool)
    .await?;
    println!("Migrations applied: {}", count);

    // Check provider constraint includes 'local'
    let has_local = sqlx::query_scalar!(
        r#"SELECT EXISTS(
            SELECT 1 FROM information_schema.check_constraints
            WHERE constraint_name = 'users_provider_check'
              AND check_clause LIKE '%local%'
        ) AS "exists!""#
    )
    .fetch_one(pool)
    .await?;
    println!(
        "Provider constraint includes 'local': {}",
        if has_local { "YES" } else { "NO — run migration 7" }
    );

    // Check system admin exists
    let has_admin = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM users WHERE system_admin = true) AS "exists!""#
    )
    .fetch_one(pool)
    .await?;
    println!(
        "System admin exists: {}",
        if has_admin { "YES" } else { "NO — run `timelord-cli create-admin`" }
    );

    Ok(())
}
