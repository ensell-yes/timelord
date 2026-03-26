use std::sync::Arc;

use chrono::{Duration, Utc};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::repo;

/// Timelord MCP tool server — exposes calendar data to AI agents.
#[derive(Clone)]
pub struct TimelordTools {
    pool: Arc<PgPool>,
    tool_router: ToolRouter<Self>,
}

#[derive(Deserialize, rmcp::schemars::JsonSchema)]
pub struct ListEventsParams {
    /// Start of time range (ISO 8601). Defaults to now.
    pub time_min: Option<String>,
    /// End of time range (ISO 8601). Defaults to 7 days from now.
    pub time_max: Option<String>,
    /// Optional calendar ID filter (UUID).
    pub calendar_id: Option<String>,
}

#[derive(Deserialize, rmcp::schemars::JsonSchema)]
pub struct SearchEventsParams {
    /// Search query (matches event title).
    pub query: String,
    /// Start of time range (ISO 8601).
    pub time_min: Option<String>,
    /// End of time range (ISO 8601).
    pub time_max: Option<String>,
}

impl TimelordTools {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self {
            pool,
            tool_router: Self::tool_router(),
        }
    }

    fn get_context(&self) -> Result<(Uuid, Uuid), String> {
        let org_id: Uuid = std::env::var("MCP_ORG_ID")
            .map_err(|_| "MCP_ORG_ID env var not set".to_string())?
            .parse()
            .map_err(|_| "MCP_ORG_ID is not a valid UUID".to_string())?;
        let user_id: Uuid = std::env::var("MCP_USER_ID")
            .map_err(|_| "MCP_USER_ID env var not set".to_string())?
            .parse()
            .map_err(|_| "MCP_USER_ID is not a valid UUID".to_string())?;
        Ok((org_id, user_id))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TimelordTools {}

#[tool_router(router = tool_router)]
impl TimelordTools {
    /// List all calendars for the current user with provider and sync status.
    #[tool(description = "List all calendars for the current user with provider and sync status")]
    async fn list_calendars(&self) -> String {
        let (org_id, user_id) = match self.get_context() {
            Ok(ctx) => ctx,
            Err(e) => return format!("Error: {e}"),
        };
        match repo::list_calendars(&self.pool, org_id, user_id).await {
            Ok(cals) => serde_json::to_string_pretty(&cals).unwrap_or_else(|e| e.to_string()),
            Err(e) => format!("Error: {e}"),
        }
    }

    /// List events in a time range with optional calendar filter.
    #[tool(description = "List events in a time range. Params: time_min, time_max (ISO 8601), calendar_id (optional UUID)")]
    async fn list_events(&self, params: Parameters<ListEventsParams>) -> String {
        let (org_id, user_id) = match self.get_context() {
            Ok(ctx) => ctx,
            Err(e) => return format!("Error: {e}"),
        };
        let now = Utc::now();
        let time_min = params
            .0
            .time_min
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(now);
        let time_max = params
            .0
            .time_max
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(now + Duration::days(7));
        let calendar_id: Option<Uuid> = params.0.calendar_id.as_ref().and_then(|s| s.parse().ok());

        match repo::list_events(&self.pool, org_id, user_id, time_min, time_max, calendar_id).await
        {
            Ok(events) => serde_json::to_string_pretty(&events).unwrap_or_else(|e| e.to_string()),
            Err(e) => format!("Error: {e}"),
        }
    }

    /// Search events by title with optional time range.
    #[tool(description = "Search events by title. Params: query (required), time_min, time_max (optional ISO 8601)")]
    async fn search_events(&self, params: Parameters<SearchEventsParams>) -> String {
        let (org_id, user_id) = match self.get_context() {
            Ok(ctx) => ctx,
            Err(e) => return format!("Error: {e}"),
        };
        let time_min = params.0.time_min.as_ref().and_then(|s| s.parse().ok());
        let time_max = params.0.time_max.as_ref().and_then(|s| s.parse().ok());

        match repo::search_events(
            &self.pool,
            org_id,
            user_id,
            &params.0.query,
            time_min,
            time_max,
        )
        .await
        {
            Ok(events) => serde_json::to_string_pretty(&events).unwrap_or_else(|e| e.to_string()),
            Err(e) => format!("Error: {e}"),
        }
    }

    /// Get pending calendar optimization suggestions from the solver.
    #[tool(description = "Get pending calendar optimization suggestions from the solver")]
    async fn get_optimization_suggestions(&self) -> String {
        let (org_id, user_id) = match self.get_context() {
            Ok(ctx) => ctx,
            Err(e) => return format!("Error: {e}"),
        };
        match repo::get_pending_suggestions(&self.pool, org_id, user_id).await {
            Ok(sug) => serde_json::to_string_pretty(&sug).unwrap_or_else(|e| e.to_string()),
            Err(e) => format!("Error: {e}"),
        }
    }
}
