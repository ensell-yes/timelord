use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{
    org::Organization,
    org_member::{OrgMember, OrgRole},
};
use timelord_common::error::AppError;

#[allow(dead_code)]
pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Organization>, AppError> {
    let org = sqlx::query_as!(
        Organization,
        "SELECT * FROM organizations WHERE id = $1",
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(org)
}

pub async fn create<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    name: &str,
    slug: &str,
    is_personal: bool,
) -> Result<Organization, AppError> {
    let org = sqlx::query_as!(
        Organization,
        r#"
        INSERT INTO organizations (name, slug, is_personal)
        VALUES ($1, $2, $3)
        RETURNING *
        "#,
        name,
        slug,
        is_personal
    )
    .fetch_one(executor)
    .await?;
    Ok(org)
}

pub async fn add_member<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    user_id: Uuid,
    role: OrgRole,
) -> Result<OrgMember, AppError> {
    let member = sqlx::query_as!(
        OrgMember,
        r#"
        INSERT INTO org_members (org_id, user_id, role)
        VALUES ($1, $2, $3)
        ON CONFLICT (org_id, user_id) DO UPDATE SET role = EXCLUDED.role, updated_at = now()
        RETURNING id, org_id, user_id, role AS "role: OrgRole", created_at, updated_at
        "#,
        org_id,
        user_id,
        role as OrgRole
    )
    .fetch_one(executor)
    .await?;
    Ok(member)
}

pub async fn get_member_role(
    pool: &PgPool,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Option<OrgRole>, AppError> {
    let row = sqlx::query!(
        r#"SELECT role AS "role: OrgRole" FROM org_members WHERE org_id = $1 AND user_id = $2"#,
        org_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.role))
}

/// List all members of an org with their user info.
pub async fn list_org_members(
    pool: &PgPool,
    org_id: Uuid,
) -> Result<Vec<serde_json::Value>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT u.id, u.email, u.display_name, u.provider, u.system_admin,
               m.role AS "role: OrgRole"
        FROM org_members m
        JOIN users u ON u.id = m.user_id
        WHERE m.org_id = $1
        ORDER BY m.created_at ASC
        "#,
        org_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "email": r.email,
                "display_name": r.display_name,
                "provider": r.provider,
                "system_admin": r.system_admin,
                "role": r.role.to_string(),
            })
        })
        .collect())
}

pub async fn list_user_orgs(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<(Organization, OrgRole)>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT o.id, o.name, o.slug, o.is_personal, o.settings, o.created_at, o.updated_at,
               m.role AS "role: OrgRole"
        FROM organizations o
        JOIN org_members m ON m.org_id = o.id
        WHERE m.user_id = $1
        ORDER BY o.created_at ASC
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let org = Organization {
                id: r.id,
                name: r.name,
                slug: r.slug,
                is_personal: r.is_personal,
                settings: r.settings,
                created_at: r.created_at,
                updated_at: r.updated_at,
            };
            (org, r.role)
        })
        .collect())
}
