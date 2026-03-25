#![allow(dead_code)]
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tonic::{Request, Response, Status};

use crate::services::{calendar_service, event_service, AppState};
use timelord_proto::timelord::calendar::{
    calendar_service_server::CalendarService, Calendar, CreateEventRequest as ProtoCreateEvent,
    DeleteEventRequest, DeleteEventResponse, Event, GetCalendarRequest, GetEventRequest,
    ListCalendarsRequest, ListCalendarsResponse, ListEventsRequest, ListEventsResponse,
    UpdateEventRequest,
};

pub struct CalendarServiceImpl {
    state: Arc<AppState>,
}

impl CalendarServiceImpl {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl CalendarService for CalendarServiceImpl {
    async fn list_calendars(
        &self,
        request: Request<ListCalendarsRequest>,
    ) -> Result<Response<ListCalendarsResponse>, Status> {
        let req = request.into_inner();
        let org_id = req
            .org_id
            .parse()
            .map_err(|_| Status::invalid_argument("invalid org_id"))?;
        let user_id = req
            .user_id
            .parse()
            .map_err(|_| Status::invalid_argument("invalid user_id"))?;

        let calendars = calendar_service::list(&self.state, org_id, user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let proto_cals = calendars
            .into_iter()
            .map(|c| Calendar {
                id: c.id.to_string(),
                org_id: c.org_id.to_string(),
                user_id: c.user_id.to_string(),
                provider: c.provider,
                provider_calendar_id: c.provider_calendar_id,
                name: c.name,
                color: c.color.unwrap_or_default(),
                is_primary: c.is_primary,
                is_visible: c.is_visible,
                timezone: c.timezone,
                sync_enabled: c.sync_enabled,
            })
            .collect();

        Ok(Response::new(ListCalendarsResponse {
            calendars: proto_cals,
            pagination: None,
        }))
    }

    async fn get_calendar(
        &self,
        request: Request<GetCalendarRequest>,
    ) -> Result<Response<Calendar>, Status> {
        let req = request.into_inner();
        let org_id = req
            .org_id
            .parse()
            .map_err(|_| Status::invalid_argument("invalid org_id"))?;
        let cal_id = req
            .calendar_id
            .parse()
            .map_err(|_| Status::invalid_argument("invalid calendar_id"))?;

        let cal = calendar_service::get(&self.state, org_id, cal_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Calendar {
            id: cal.id.to_string(),
            org_id: cal.org_id.to_string(),
            user_id: cal.user_id.to_string(),
            provider: cal.provider,
            provider_calendar_id: cal.provider_calendar_id,
            name: cal.name,
            color: cal.color.unwrap_or_default(),
            is_primary: cal.is_primary,
            is_visible: cal.is_visible,
            timezone: cal.timezone,
            sync_enabled: cal.sync_enabled,
        }))
    }

    async fn list_events(
        &self,
        request: Request<ListEventsRequest>,
    ) -> Result<Response<ListEventsResponse>, Status> {
        let req = request.into_inner();
        let org_id = req
            .org_id
            .parse()
            .map_err(|_| Status::invalid_argument("invalid org_id"))?;
        let cal_id = req
            .calendar_id
            .parse()
            .map_err(|_| Status::invalid_argument("invalid calendar_id"))?;

        let time_min = if req.time_min_unix > 0 {
            DateTime::<Utc>::from_timestamp(req.time_min_unix, 0)
        } else {
            None
        };
        let time_max = if req.time_max_unix > 0 {
            DateTime::<Utc>::from_timestamp(req.time_max_unix, 0)
        } else {
            None
        };

        let events = event_service::list(&self.state, org_id, cal_id, time_min, time_max, 50, 0)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let proto_events = events.into_iter().map(event_to_proto).collect();

        Ok(Response::new(ListEventsResponse {
            events: proto_events,
            pagination: None,
        }))
    }

    async fn get_event(
        &self,
        request: Request<GetEventRequest>,
    ) -> Result<Response<Event>, Status> {
        let req = request.into_inner();
        let org_id = req
            .org_id
            .parse()
            .map_err(|_| Status::invalid_argument("invalid org_id"))?;
        let event_id = req
            .event_id
            .parse()
            .map_err(|_| Status::invalid_argument("invalid event_id"))?;

        let event = event_service::get(&self.state, org_id, event_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(event_to_proto(event)))
    }

    async fn create_event(
        &self,
        _request: Request<ProtoCreateEvent>,
    ) -> Result<Response<Event>, Status> {
        Err(Status::unimplemented("use REST API for event creation"))
    }

    async fn update_event(
        &self,
        _request: Request<UpdateEventRequest>,
    ) -> Result<Response<Event>, Status> {
        Err(Status::unimplemented("use REST API for event updates"))
    }

    async fn delete_event(
        &self,
        _request: Request<DeleteEventRequest>,
    ) -> Result<Response<DeleteEventResponse>, Status> {
        Err(Status::unimplemented("use REST API for event deletion"))
    }
}

fn event_to_proto(e: crate::models::event::Event) -> Event {
    Event {
        id: e.id.to_string(),
        org_id: e.org_id.to_string(),
        calendar_id: e.calendar_id.to_string(),
        title: e.title,
        description: e.description.unwrap_or_default(),
        location: e.location.unwrap_or_default(),
        start_at_unix: e.start_at.timestamp(),
        end_at_unix: e.end_at.timestamp(),
        all_day: e.all_day,
        timezone: e.timezone,
        status: format!("{:?}", e.status).to_lowercase(),
        visibility: format!("{:?}", e.visibility).to_lowercase(),
        is_organizer: e.is_organizer,
        organizer_email: e.organizer_email.unwrap_or_default(),
        self_rsvp_status: format!("{:?}", e.self_rsvp_status).to_lowercase(),
        attendees_json: e.attendees.to_string(),
        recurrence_rule: e.recurrence_rule.unwrap_or_default(),
        is_movable: e.is_movable,
        is_heads_down: e.is_heads_down,
    }
}
