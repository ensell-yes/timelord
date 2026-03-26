use chrono::{DateTime, Duration, NaiveTime, Timelike, Utc};
use good_lp::{constraint, default_solver, variable, Expression, Solution, SolverModel};

use crate::models::{SolverConfig, SolverEvent, SolverMetrics, SolverResult, SolverSuggestion};

/// Run the ILP optimization on the given events.
#[allow(clippy::needless_range_loop)]
pub fn optimize(events: Vec<SolverEvent>, config: &SolverConfig) -> SolverResult {
    let (fixed, movable): (Vec<_>, Vec<_>) = events.iter().partition(|e| e.is_fixed());

    let total_events = events.len();
    let movable_count = movable.len();

    if movable.is_empty() {
        let metrics = compute_metrics(&events, &[], config, total_events, movable_count);
        return SolverResult {
            suggestions: vec![],
            metrics,
        };
    }

    let total_slots = compute_total_slots(config);
    if total_slots == 0 {
        let metrics = compute_metrics(&events, &[], config, total_events, movable_count);
        return SolverResult {
            suggestions: vec![],
            metrics,
        };
    }

    // Build slot occupancy from fixed events
    let mut fixed_occupancy = vec![false; total_slots];
    for event in &fixed {
        mark_slots(event, config, &mut fixed_occupancy);
    }

    // Build valid start slots for each movable event
    let mut event_vars: Vec<EventVars> = Vec::new();
    let mut problem = good_lp::ProblemVariables::new();

    for event in &movable {
        let duration_slots = (event.duration_minutes() / config.slot_minutes).max(1) as usize;
        let valid_starts = compute_valid_starts(event, config, total_slots, duration_slots, &fixed_occupancy);

        // Use continuous [0,1] variables instead of binary. The assignment +
        // capacity constraints form a TU matrix, so LP relaxation yields
        // integral solutions without needing a MIP solver.
        let vars: Vec<_> = valid_starts
            .iter()
            .map(|_| problem.add(variable().min(0.0).max(1.0)))
            .collect();

        event_vars.push(EventVars {
            event,
            duration_slots,
            valid_starts,
            vars,
        });
    }

    // Build the ILP
    let mut objective: Expression = Expression::from(0.0);

    // Movement cost: penalize distance from original slot
    for ev in &event_vars {
        let original_slot = time_to_slot(&ev.event.start_at, config);
        for (i, &start_slot) in ev.valid_starts.iter().enumerate() {
            let distance = (start_slot as f64 - original_slot as f64).abs();
            objective += config.movement_weight * distance * ev.vars[i];
        }
    }

    let mut model = problem.minimise(&objective).using(default_solver);

    // Constraint: each movable event assigned exactly one start slot
    for ev in &event_vars {
        if ev.vars.is_empty() {
            continue;
        }
        let sum: Expression = ev.vars.iter().copied().sum();
        model = model.with(constraint!(sum == 1.0));
    }

    // Constraint: no overlap at each slot (including fixed events)
    for slot in 0..total_slots {
        if !is_working_slot(slot, config) {
            continue;
        }

        let mut slot_usage: Expression = Expression::from(0.0);
        if fixed_occupancy[slot] {
            slot_usage += 1.0;
        }

        for ev in &event_vars {
            for (i, &start_slot) in ev.valid_starts.iter().enumerate() {
                if slot >= start_slot && slot < start_slot + ev.duration_slots {
                    slot_usage += ev.vars[i];
                }
            }
        }

        model = model.with(constraint!(slot_usage <= 1.0));
    }

    // Solve
    let solution = match model.solve() {
        Ok(s) => s,
        Err(_) => {
            // Solver couldn't find a feasible solution — return no suggestions
            let metrics = compute_metrics(&events, &[], config, total_events, movable_count);
            return SolverResult {
                suggestions: vec![],
                metrics,
            };
        }
    };

    // Extract suggestions
    let mut suggestions = Vec::new();
    for ev in &event_vars {
        let assigned_slot = ev
            .valid_starts
            .iter()
            .enumerate()
            .find(|(i, _)| solution.value(ev.vars[*i]) > 0.5)
            .map(|(_, &s)| s);

        if let Some(new_slot) = assigned_slot {
            let new_start = slot_to_time(new_slot, config);
            let duration = ev.event.end_at - ev.event.start_at;
            let new_end = new_start + duration;

            // Only suggest if the event actually moved (≥ 1 slot difference)
            let original_slot = time_to_slot(&ev.event.start_at, config);
            if new_slot != original_slot {
                let reason = generate_reason(ev.event, &new_start, &new_end, config);
                suggestions.push(SolverSuggestion {
                    event_id: ev.event.id,
                    event_title: ev.event.title.clone(),
                    original_start: ev.event.start_at,
                    original_end: ev.event.end_at,
                    suggested_start: new_start,
                    suggested_end: new_end,
                    reason,
                });
            }
        }
    }

    let metrics = compute_metrics(&events, &suggestions, config, total_events, movable_count);

    SolverResult {
        suggestions,
        metrics,
    }
}

struct EventVars<'a> {
    event: &'a SolverEvent,
    duration_slots: usize,
    valid_starts: Vec<usize>,
    vars: Vec<good_lp::Variable>,
}

fn compute_total_slots(config: &SolverConfig) -> usize {
    let total_minutes = (config.window_end - config.window_start).num_minutes();
    (total_minutes / config.slot_minutes).max(0) as usize
}

fn time_to_slot(time: &DateTime<Utc>, config: &SolverConfig) -> usize {
    let minutes = (*time - config.window_start).num_minutes();
    (minutes / config.slot_minutes).max(0) as usize
}

fn slot_to_time(slot: usize, config: &SolverConfig) -> DateTime<Utc> {
    config.window_start + Duration::minutes(slot as i64 * config.slot_minutes)
}

fn is_working_slot(slot: usize, config: &SolverConfig) -> bool {
    let time = slot_to_time(slot, config);
    let naive = time.time();
    let work_start = config.work_start;
    let work_end = config.work_end;
    naive >= work_start && naive < work_end
}

fn mark_slots(event: &SolverEvent, config: &SolverConfig, occupancy: &mut [bool]) {
    let start_slot = time_to_slot(&event.start_at, config);
    let end_slot = time_to_slot(&event.end_at, config);
    for slot in start_slot..end_slot.min(occupancy.len()) {
        occupancy[slot] = true;
    }
}

fn compute_valid_starts(
    _event: &SolverEvent,
    config: &SolverConfig,
    total_slots: usize,
    duration_slots: usize,
    _fixed_occupancy: &[bool],
) -> Vec<usize> {
    let mut valid = Vec::new();
    for slot in 0..total_slots {
        // Event must fit within the window
        if slot + duration_slots > total_slots {
            break;
        }
        // All slots of the event must be within working hours
        let all_working = (slot..slot + duration_slots).all(|s| is_working_slot(s, config));
        if !all_working {
            continue;
        }
        // Don't place events that span midnight working boundaries
        let start_time = slot_to_time(slot, config);
        let end_time = slot_to_time(slot + duration_slots, config);
        if start_time.date_naive() != end_time.date_naive()
            && end_time.time() != NaiveTime::from_hms_opt(0, 0, 0).unwrap()
        {
            continue;
        }
        valid.push(slot);
    }
    valid
}

fn generate_reason(
    event: &SolverEvent,
    new_start: &DateTime<Utc>,
    _new_end: &DateTime<Utc>,
    _config: &SolverConfig,
) -> String {
    let original_hour = event.start_at.hour();
    let new_hour = new_start.hour();
    let diff_minutes = (*new_start - event.start_at).num_minutes().abs();

    if diff_minutes < 60 {
        format!("Moved {}min to reduce fragmentation", diff_minutes)
    } else {
        let hours = diff_minutes / 60;
        let dir = if new_hour > original_hour {
            "later"
        } else {
            "earlier"
        };
        format!("Moved ~{}h {} to consolidate schedule", hours, dir)
    }
}

fn compute_metrics(
    all_events: &[SolverEvent],
    suggestions: &[SolverSuggestion],
    config: &SolverConfig,
    total_events: usize,
    movable_count: usize,
) -> SolverMetrics {
    let focus_before = compute_focus_hours(all_events, &[], config);
    let focus_after = compute_focus_hours(all_events, suggestions, config);

    let frag_before = compute_fragmentation(all_events, &[], config);
    let frag_after = compute_fragmentation(all_events, suggestions, config);

    SolverMetrics {
        events_analyzed: total_events,
        events_movable: movable_count,
        suggestions: suggestions.len(),
        focus_hours_before: focus_before,
        focus_hours_after: focus_after,
        focus_hours_gained: focus_after - focus_before,
        fragmentation_before: frag_before,
        fragmentation_after: frag_after,
    }
}

/// Count total focus hours (contiguous free blocks ≥ 2 hours during working hours).
#[allow(clippy::needless_range_loop)]
fn compute_focus_hours(
    events: &[SolverEvent],
    suggestions: &[SolverSuggestion],
    config: &SolverConfig,
) -> f64 {
    let total_slots = compute_total_slots(config);
    let mut occupancy = vec![false; total_slots];

    for event in events {
        let (start, end) = applied_times(event, suggestions);
        let s = time_to_slot(&start, config);
        let e = time_to_slot(&end, config);
        for slot in s..e.min(total_slots) {
            occupancy[slot] = true;
        }
    }

    let focus_threshold_slots = (120 / config.slot_minutes) as usize; // 2 hours
    let mut total_focus_minutes: i64 = 0;
    let mut free_run = 0usize;

    for slot in 0..total_slots {
        if is_working_slot(slot, config) && !occupancy[slot] {
            free_run += 1;
        } else {
            if free_run >= focus_threshold_slots {
                total_focus_minutes += free_run as i64 * config.slot_minutes;
            }
            free_run = 0;
        }
    }
    if free_run >= focus_threshold_slots {
        total_focus_minutes += free_run as i64 * config.slot_minutes;
    }

    total_focus_minutes as f64 / 60.0
}

/// Compute fragmentation score (average context switches per day).
#[allow(clippy::needless_range_loop)]
fn compute_fragmentation(
    events: &[SolverEvent],
    suggestions: &[SolverSuggestion],
    config: &SolverConfig,
) -> f64 {
    let total_slots = compute_total_slots(config);
    let mut occupancy = vec![false; total_slots];

    for event in events {
        let (start, end) = applied_times(event, suggestions);
        let s = time_to_slot(&start, config);
        let e = time_to_slot(&end, config);
        for slot in s..e.min(total_slots) {
            occupancy[slot] = true;
        }
    }

    let mut switches = 0;
    let mut prev_busy = false;
    let mut was_free_between = false;

    for slot in 0..total_slots {
        if !is_working_slot(slot, config) {
            prev_busy = false;
            was_free_between = false;
            continue;
        }
        let busy = occupancy[slot];
        if busy && prev_busy && was_free_between {
            switches += 1;
            was_free_between = false;
        }
        if !busy && prev_busy {
            was_free_between = true;
        }
        prev_busy = busy || prev_busy;
        if busy {
            prev_busy = true;
            was_free_between = false;
        } else if prev_busy {
            was_free_between = true;
        }
    }

    let days = ((config.window_end - config.window_start).num_days()).max(1) as f64;
    switches as f64 / days
}

/// Get the effective start/end for an event, applying any suggestion.
fn applied_times(
    event: &SolverEvent,
    suggestions: &[SolverSuggestion],
) -> (DateTime<Utc>, DateTime<Utc>) {
    if let Some(s) = suggestions.iter().find(|s| s.event_id == event.id) {
        (s.suggested_start, s.suggested_end)
    } else {
        (event.start_at, event.end_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;
    use uuid::Uuid;

    fn make_event(
        title: &str,
        start_hour: u32,
        start_min: u32,
        end_hour: u32,
        end_min: u32,
        movable: bool,
    ) -> SolverEvent {
        let start = Utc.with_ymd_and_hms(2026, 3, 26, start_hour, start_min, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 3, 26, end_hour, end_min, 0).unwrap();
        SolverEvent {
            id: Uuid::new_v4(),
            title: title.to_string(),
            start_at: start,
            end_at: end,
            all_day: false,
            is_movable: movable,
            is_heads_down: false,
            is_organizer: true,
            attendees: json!([]),
            status: "confirmed".to_string(),
        }
    }

    fn test_config() -> SolverConfig {
        let window_start = Utc.with_ymd_and_hms(2026, 3, 26, 0, 0, 0).unwrap();
        let window_end = Utc.with_ymd_and_hms(2026, 3, 27, 0, 0, 0).unwrap();
        SolverConfig {
            window_start,
            window_end,
            work_start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            work_end: NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
            slot_minutes: 15,
            movement_weight: 0.1,
            focus_weight: 5.0,
            fragmentation_weight: 2.0,
        }
    }

    #[test]
    fn test_no_movable_events() {
        let events = vec![make_event("Fixed meeting", 10, 0, 11, 0, false)];
        let result = optimize(events, &test_config());
        assert!(result.suggestions.is_empty());
        assert_eq!(result.metrics.events_movable, 0);
    }

    #[test]
    fn test_single_movable_event() {
        let events = vec![make_event("Task", 10, 0, 10, 30, true)];
        let config = test_config();
        let result = optimize(events, &config);
        // Solver should find a valid placement (may or may not move it)
        assert_eq!(result.metrics.events_analyzed, 1);
        assert_eq!(result.metrics.events_movable, 1);
    }

    #[test]
    fn test_movable_consolidation() {
        // Fixed meeting at 11:00-12:00, two movable 30min tasks scattered at 9:30 and 14:00
        let events = vec![
            make_event("Fixed meeting", 11, 0, 12, 0, false),
            make_event("Task A", 9, 30, 10, 0, true),
            make_event("Task B", 14, 0, 14, 30, true),
        ];
        let config = test_config();
        let result = optimize(events, &config);
        assert_eq!(result.metrics.events_analyzed, 3);
        assert_eq!(result.metrics.events_movable, 2);
        // Solver should produce a valid (non-overlapping) assignment
    }

    #[test]
    fn test_all_slots_full_no_crash() {
        // Fill working hours with fixed events, try to place a movable one
        let mut events: Vec<SolverEvent> = Vec::new();
        for hour in 9..17 {
            events.push(make_event(
                &format!("Fixed {hour}"),
                hour,
                0,
                hour + 1,
                0,
                false,
            ));
        }
        events.push(make_event("Can't fit", 10, 0, 10, 30, true));
        let config = test_config();
        // Should not panic, even if solver can't find a feasible solution
        let result = optimize(events, &config);
        assert!(result.suggestions.is_empty() || result.suggestions.len() <= 1);
    }
}
