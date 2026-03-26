use std::sync::Arc;
use tokio::time::{interval, Duration};

use async_nats::Client as NatsClient;
use chrono::Utc;
use sqlx::PgPool;

use crate::{
    config::Config,
    provider::{self, google, microsoft},
    repo::{calendar_repo, event_repo, sync_state_repo},
};
use timelord_common::{
    audit::{insert_audit, AuditEntry},
    db,
    error::AppError,
    provider_token, token_encryption::TokenEncryptor, token_refresh,
};

pub async fn run_sync_loop(
    pool: PgPool,
    nats: NatsClient,
    http: reqwest::Client,
    encryptor: Arc<TokenEncryptor>,
    config: Arc<Config>,
) {
    let mut tick = interval(Duration::from_secs(config.sync_interval_secs));
    tracing::info!(
        interval_secs = config.sync_interval_secs,
        "sync loop started"
    );

    loop {
        tick.tick().await;
        tracing::debug!("sync loop tick");

        let work_items = match calendar_repo::list_sync_work_items(&pool).await {
            Ok(items) => items,
            Err(e) => {
                tracing::error!(error = %e, "failed to list sync work items");
                continue;
            }
        };

        for item in &work_items {
            if let Err(e) = sync_one_calendar(
                &pool, &nats, &http, &encryptor, &config, item,
            )
            .await
            {
                tracing::warn!(
                    calendar_id = %item.calendar_id,
                    provider = %item.provider,
                    error = %e,
                    "sync failed for calendar"
                );
                // Record error in sync_state with RLS context (best-effort)
                if let Ok(mut tx) = pool.begin().await {
                    if db::set_rls_context(&mut tx, item.org_id).await.is_ok() {
                        let _ = sync_state_repo::record_error(
                            &mut tx,
                            item.calendar_id,
                            &e.to_string(),
                        )
                        .await;
                        let _ = tx.commit().await;
                    }
                }
            }
        }
    }
}

async fn sync_one_calendar(
    pool: &PgPool,
    nats: &NatsClient,
    http: &reqwest::Client,
    encryptor: &TokenEncryptor,
    config: &Config,
    item: &calendar_repo::SyncWorkItem,
) -> Result<(), AppError> {
    // --- Token acquisition (short transaction with RLS context) ---
    let access_token = acquire_access_token(pool, http, encryptor, config, item).await?;

    // --- Event fetch (outside any transaction) ---
    let sync_result = match fetch_events(http, &access_token, item).await {
        Ok(r) => r,
        Err(AppError::SyncTokenInvalid) => {
            tracing::warn!(
                calendar_id = %item.calendar_id,
                "sync token invalidated, clearing for full re-sync next iteration"
            );
            let mut tx = pool.begin().await.map_err(AppError::internal)?;
            db::set_rls_context(&mut tx, item.org_id).await.map_err(AppError::internal)?;
            sync_state_repo::clear_sync_token(&mut tx, item.calendar_id).await?;
            tx.commit().await.map_err(AppError::internal)?;
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    // --- Upsert (transaction with RLS context) ---
    let mut mutations = 0u32;
    let mut cancelled = 0u32;

    let mut tx = pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, item.org_id).await.map_err(AppError::internal)?;

    for event in &sync_result.events {
        if event.status == "cancelled" {
            cancelled += 1;
        } else {
            mutations += 1;
        }
        event_repo::upsert_event(&mut tx, item.org_id, item.calendar_id, event).await?;
    }

    // Update sync state
    sync_state_repo::update_after_sync(
        &mut tx,
        item.calendar_id,
        sync_result.next_sync_token.as_deref(),
        sync_result.events.len() as i32,
    )
    .await?;

    // Audit inside transaction (RLS context already set)
    if !sync_result.events.is_empty() {
        insert_audit(
            &mut *tx,
            AuditEntry::new(item.org_id, "sync", "calendar")
                .entity(item.calendar_id)
                .meta(serde_json::json!({
                    "mutations": mutations,
                    "cancelled": cancelled,
                    "total": sync_result.events.len(),
                })),
        )
        .await;
    }

    tx.commit().await.map_err(AppError::internal)?;

    // Publish NATS events (after commit)
    if mutations > 0 {
        publish_nats(nats, item, "synced", mutations).await;
    }
    if cancelled > 0 {
        publish_nats(nats, item, "cancelled", cancelled).await;
    }

    tracing::info!(
        calendar_id = %item.calendar_id,
        provider = %item.provider,
        events = sync_result.events.len(),
        "sync completed"
    );

    Ok(())
}

async fn acquire_access_token(
    pool: &PgPool,
    http: &reqwest::Client,
    encryptor: &TokenEncryptor,
    config: &Config,
    item: &calendar_repo::SyncWorkItem,
) -> Result<String, AppError> {
    // Always read inside a transaction with RLS context
    let mut tx = pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, item.org_id).await.map_err(AppError::internal)?;

    let token = provider_token::find_for_user_locked(&mut tx, item.user_id, &item.provider)
        .await?
        .ok_or_else(|| {
            AppError::internal(format!(
                "No provider token for user {} provider {}",
                item.user_id, item.provider
            ))
        })?;

    if token.expires_at > Utc::now() {
        let access_nonce = &token.token_nonce[..12];
        let decrypted = encryptor.decrypt(&token.access_token_enc, access_nonce)?;
        tx.commit().await.map_err(AppError::internal)?;
        return Ok(decrypted);
    }

    // Expired — decrypt refresh token, release lock before HTTP call
    let refresh_nonce = &token.token_nonce[12..];
    let refresh_token_plain = encryptor.decrypt(&token.refresh_token_enc, refresh_nonce)?;
    tx.commit().await.map_err(AppError::internal)?;

    let result = match item.provider.as_str() {
        "google" => {
            token_refresh::refresh_google_token(
                http,
                &config.google_client_id,
                &config.google_client_secret,
                &refresh_token_plain,
            )
            .await?
        }
        "microsoft" => {
            token_refresh::refresh_microsoft_token(
                http,
                &config.microsoft_client_id,
                &config.microsoft_client_secret,
                &config.microsoft_tenant_id,
                &refresh_token_plain,
            )
            .await?
        }
        _ => {
            return Err(AppError::internal(format!(
                "Unknown provider: {}",
                item.provider
            )))
        }
    };

    // Re-encrypt and update in a new locked transaction
    let new_refresh = result
        .refresh_token
        .as_deref()
        .unwrap_or(&refresh_token_plain);
    let (access_enc, access_nonce_new) = encryptor.encrypt(&result.access_token)?;
    let (refresh_enc, refresh_nonce_new) = encryptor.encrypt(new_refresh)?;
    let mut combined_nonce = access_nonce_new;
    combined_nonce.extend_from_slice(&refresh_nonce_new);
    let new_expires_at = Utc::now() + chrono::Duration::seconds(result.expires_in_secs as i64);

    let mut tx = pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, item.org_id).await.map_err(AppError::internal)?;

    let locked = provider_token::find_for_user_locked(&mut tx, item.user_id, &item.provider)
        .await?
        .ok_or_else(|| {
            AppError::internal(format!(
                "Provider token disappeared during refresh for user {} provider {}",
                item.user_id, item.provider
            ))
        })?;

    provider_token::update_tokens(
        &mut tx,
        locked.id,
        &access_enc,
        &refresh_enc,
        &combined_nonce,
        new_expires_at,
    )
    .await?;
    tx.commit().await.map_err(AppError::internal)?;

    tracing::info!(
        user_id = %item.user_id,
        provider = %item.provider,
        "refreshed provider token"
    );

    Ok(result.access_token)
}

async fn fetch_events(
    http: &reqwest::Client,
    access_token: &str,
    item: &calendar_repo::SyncWorkItem,
) -> Result<provider::SyncResult, AppError> {
    match item.provider.as_str() {
        "google" => {
            google::fetch_google_events(
                http,
                access_token,
                &item.provider_calendar_id,
                item.sync_token.as_deref(),
            )
            .await
        }
        "microsoft" => {
            microsoft::fetch_microsoft_events(
                http,
                access_token,
                &item.provider_calendar_id,
                item.sync_token.as_deref(),
            )
            .await
        }
        _ => Err(AppError::internal(format!(
            "Unknown provider: {}",
            item.provider
        ))),
    }
}

async fn publish_nats(
    nats: &NatsClient,
    item: &calendar_repo::SyncWorkItem,
    action: &str,
    count: u32,
) {
    let subject = format!("timelord.event.{action}");
    let body = serde_json::json!({
        "org_id": item.org_id,
        "calendar_id": item.calendar_id,
        "provider": item.provider,
        "count": count,
    });
    let bytes = match serde_json::to_vec(&body) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "failed to serialize NATS sync event");
            return;
        }
    };
    if let Err(e) = nats.publish(subject.clone(), bytes.into()).await {
        tracing::warn!(error = %e, subject = %subject, "failed to publish NATS sync event");
    }
}
