use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::routes::GatewayState;

pub async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "timelord-gateway" }))
}

pub async fn root(State(state): State<Arc<GatewayState>>) -> Json<Value> {
    let setup_status = match state
        .http_client
        .get(format!(
            "{}/setup/status",
            state.config.auth_service_http_url
        ))
        .send()
        .await
    {
        Ok(resp) => resp.json::<Value>().await.ok(),
        Err(_) => None,
    };

    let (setup_complete, has_admin) = setup_status
        .as_ref()
        .map(|s| {
            (
                s["setup_complete"].as_bool().unwrap_or(false),
                s["has_admin"].as_bool().unwrap_or(false),
            )
        })
        .unwrap_or((false, false));

    Json(json!({
        "service": "timelord",
        "version": env!("CARGO_PKG_VERSION"),
        "setup_complete": setup_complete,
        "has_admin": has_admin,
        "onboarding": if !setup_complete {
            json!({
                "required": true,
                "steps": onboarding_steps(has_admin),
                "next": if !has_admin {
                    "/auth/google"
                } else {
                    "/setup/status"
                }
            })
        } else {
            json!({ "required": false })
        }
    }))
}

fn onboarding_steps(has_admin: bool) -> Vec<Value> {
    let mut steps = vec![];

    if !has_admin {
        steps.push(json!({
            "step": 1,
            "key": "create_admin",
            "title": "Sign in to create admin account",
            "endpoint": "/auth/google",
            "complete": false,
        }));
    } else {
        steps.push(json!({
            "step": 1,
            "key": "create_admin",
            "title": "Admin account created",
            "complete": true,
        }));
    }

    steps.push(json!({
        "step": 2,
        "key": "connect_calendar",
        "title": "Connect a calendar provider",
        "endpoint": "/api/v1/providers",
        "complete": false,
    }));

    steps.push(json!({
        "step": 3,
        "key": "initial_sync",
        "title": "Sync your calendars",
        "endpoint": "/api/v1/sync",
        "complete": false,
    }));

    steps
}
