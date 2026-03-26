use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Subset of Event fields needed for optimization.
#[derive(Debug, Clone)]
pub struct SolverEvent {
    pub id: Uuid,
    pub title: String,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub all_day: bool,
    pub is_movable: bool,
    #[allow(dead_code)]
    pub is_heads_down: bool,
    pub is_organizer: bool,
    pub attendees: Value,
    pub status: String,
}

impl SolverEvent {
    /// Determine if this event should be treated as fixed (immovable) by the solver.
    pub fn is_fixed(&self) -> bool {
        !self.is_movable
            || self.all_day
            || self.status != "confirmed"
            || (self.has_attendees() && !self.is_organizer)
    }

    fn has_attendees(&self) -> bool {
        match &self.attendees {
            Value::Array(arr) => !arr.is_empty(),
            _ => false,
        }
    }

    /// Duration in minutes.
    pub fn duration_minutes(&self) -> i64 {
        (self.end_at - self.start_at).num_minutes()
    }
}

#[derive(Debug, Deserialize)]
pub struct OptimizeRequest {
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub working_hours_start: Option<String>, // "HH:MM" format
    pub working_hours_end: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SolverConfig {
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub work_start: NaiveTime,
    pub work_end: NaiveTime,
    pub slot_minutes: i64,
    /// Weight for penalizing how far an event moves from its original time.
    pub movement_weight: f64,
    /// Weight for rewarding focus blocks (contiguous free time ≥ 2 hours).
    #[allow(dead_code)]
    pub focus_weight: f64,
    /// Weight for penalizing fragmentation (busy-free-busy transitions).
    #[allow(dead_code)]
    pub fragmentation_weight: f64,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            window_start: Utc::now(),
            window_end: Utc::now(),
            work_start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            work_end: NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
            slot_minutes: 15,
            movement_weight: 0.1,
            focus_weight: 5.0,
            fragmentation_weight: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SolverSuggestion {
    pub event_id: Uuid,
    pub event_title: String,
    pub original_start: DateTime<Utc>,
    pub original_end: DateTime<Utc>,
    pub suggested_start: DateTime<Utc>,
    pub suggested_end: DateTime<Utc>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SolverMetrics {
    pub events_analyzed: usize,
    pub events_movable: usize,
    pub suggestions: usize,
    pub focus_hours_before: f64,
    pub focus_hours_after: f64,
    pub focus_hours_gained: f64,
    pub fragmentation_before: f64,
    pub fragmentation_after: f64,
}

#[derive(Debug)]
pub struct SolverResult {
    pub suggestions: Vec<SolverSuggestion>,
    pub metrics: SolverMetrics,
}

// DB models

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct OptimizationRun {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub status: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub config: Value,
    pub metrics: Option<Value>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct OptimizationSuggestion {
    pub id: Uuid,
    pub run_id: Uuid,
    pub org_id: Uuid,
    pub event_id: Uuid,
    pub original_start: DateTime<Utc>,
    pub original_end: DateTime<Utc>,
    pub suggested_start: DateTime<Utc>,
    pub suggested_end: DateTime<Utc>,
    pub reason: Option<String>,
    pub applied: bool,
    pub applied_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ApplyRequest {
    pub suggestion_ids: Vec<Uuid>,
}
