//! Tests de l'API de bout en bout : requête HTTP → router → use case → dépôts en mémoire.
//! Le router est appelé directement (`oneshot`), sans ouvrir de port réseau.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use chrono::NaiveDateTime;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::{
    application::fakes::{
        FailingRepository, FakeNotifier, FixedClock, InMemoryBookingRepository, InMemoryRoomRepository,
        InMemoryUserRepository, Notification,
    },
    domain::{
        booking::{
            Booking, BookingStatus,
            tests::{amphi, booking, event_date, requested_at, salle_101, t},
        },
        user::{User, UserRole},
    },
    infrastructure::{
        http::{USER_ID_HEADER, router},
        state::{Adapters, AppState},
    },
};

struct TestAdapters;

impl Adapters for TestAdapters {
    type Users = InMemoryUserRepository;
    type Rooms = InMemoryRoomRepository;
    type Bookings = InMemoryBookingRepository;
    type Notifier = FakeNotifier;
    type Clock = FixedClock;
}

/// Comme `TestAdapters`, mais le stockage des réservations est en panne.
struct BrokenStorage;

impl Adapters for BrokenStorage {
    type Users = InMemoryUserRepository;
    type Rooms = InMemoryRoomRepository;
    type Bookings = FailingRepository;
    type Notifier = FakeNotifier;
    type Clock = FixedClock;
}

fn users() -> InMemoryUserRepository {
    InMemoryUserRepository::with(vec![
        User::new("A-001".into(), "User1".into(), UserRole::Applicant),
        User::new("A-002".into(), "User2".into(), UserRole::Applicant),
        User::new("RESP-001".into(), "Chef".into(), UserRole::Responsible),
    ])
}

fn rooms() -> InMemoryRoomRepository {
    InMemoryRoomRepository::with(vec![salle_101(), amphi()])
}

/// État de test avec l'horloge figée à `now` et des réservations déjà enregistrées.
fn app_at(now: NaiveDateTime, bookings: Vec<Booking>) -> Arc<AppState<TestAdapters>> {
    Arc::new(AppState::new(
        users(),
        rooms(),
        InMemoryBookingRepository::with(bookings),
        FakeNotifier::default(),
        FixedClock(now),
    ))
}

/// Au moment de la demande (6 jours avant l'événement, R4 respectée), aucune réservation.
fn app() -> Arc<AppState<TestAdapters>> {
    app_at(requested_at(), vec![])
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    json: Value,
}

async fn call<A: Adapters>(
    state: &Arc<AppState<A>>,
    method: Method,
    uri: &str,
    user: Option<&str>,
    body: Option<String>,
) -> Reply {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(user) = user {
        request = request.header(USER_ID_HEADER, user);
    }
    let request = match body {
        Some(body) => request.header(header::CONTENT_TYPE, "application/json").body(Body::from(body)),
        None => request.body(Body::empty()),
    }
    .unwrap();

    let response = router(state.clone()).oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap() };
    Reply { status, headers, json }
}

/// Demande de salle(s) le jour de l'événement de test, faite par `user`.
async fn submit<A: Adapters>(state: &Arc<AppState<A>>, user: &str, rooms: &[&str], participants: u8, start: &str, end: &str) -> Reply {
    let body = json!({
        "room_ids": rooms,
        "participants": participants,
        "event_date": event_date(),
        "event_start": start,
        "event_end": end,
    });
    call(state, Method::POST, "/bookings", Some(user), Some(body.to_string())).await
}

/// Réservation B-001 de A-001, approuvée (salles bloquées), 10h–12h en salle 101.
fn approved_booking() -> Booking {
    let mut b = booking("B-001", salle_101(), BookingStatus::Requested, t(10, 0), t(12, 0));
    b.approve();
    b
}

// ---------- catalogue ----------

#[tokio::test]
async fn rooms_are_listed_without_authentication() {
    let reply = call(&app(), Method::GET, "/rooms", None, None).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        reply.json,
        json!([
            { "id": "R-101", "name": "Salle 101", "capacity": 30, "kind": "standard" },
            { "id": "R-001", "name": "Amphi A", "capacity": 100, "kind": "amphi" },
        ])
    );
}

// ---------- soumission ----------

#[tokio::test]
async fn standard_room_booking_is_created_and_confirmed() {
    let state = app();

    let reply = submit(&state, "A-001", &["R-101"], 20, "10:00", "11:00").await;

    assert_eq!(reply.status, StatusCode::CREATED);
    assert_eq!(reply.json["outcome"], "confirmed");
    let booking = &reply.json["booking"];
    assert_eq!(booking["status"], "approved");
    assert_eq!(booking["applicant_id"], "A-001");
    assert_eq!(booking["room_ids"], json!(["R-101"]));
    assert_eq!(booking["event_start"], "10:00:00");
    let id = booking["id"].as_str().unwrap();
    assert_eq!(reply.headers[header::LOCATION], format!("/bookings/{id}"));
    assert_eq!(state.notifier.sent(), vec![Notification::Confirmed(id.into())]);
}

#[tokio::test]
async fn amphi_booking_is_created_pending_approval() {
    let reply = submit(&app(), "A-001", &["R-001"], 80, "14:00", "16:00").await;

    assert_eq!(reply.status, StatusCode::CREATED);
    assert_eq!(reply.json["outcome"], "pending_approval");
    assert_eq!(reply.json["booking"]["status"], "requested");
}

#[tokio::test]
async fn missing_user_header_is_401() {
    let body = json!({ "room_ids": ["R-101"], "participants": 20, "event_date": event_date(), "event_start": "10:00", "event_end": "11:00" });

    let reply = call(&app(), Method::POST, "/bookings", None, Some(body.to_string())).await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.json["error"], "missing_user");
}

#[tokio::test]
async fn unknown_user_is_401() {
    let reply = submit(&app(), "U-404", &["R-101"], 20, "10:00", "11:00").await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.json["error"], "unknown_user");
}

#[tokio::test]
async fn taken_slot_is_409_with_alternative_slots() {
    let state = app();
    submit(&state, "A-001", &["R-101"], 20, "10:00", "11:00").await;

    let reply = submit(&state, "A-002", &["R-101"], 20, "10:00", "11:00").await;

    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.json["error"], "slot_unavailable");
    let alternatives = reply.json["alternatives"].as_array().unwrap();
    assert!(!alternatives.is_empty());
    assert!(alternatives.contains(&json!({ "start": "11:00:00", "end": "12:00:00" })));
}

#[tokio::test]
async fn capacity_exceeded_is_422() {
    let reply = submit(&app(), "A-001", &["R-101"], 40, "10:00", "11:00").await;

    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.json["error"], "capacity_exceeded");
}

#[tokio::test]
async fn unknown_room_is_422() {
    let reply = submit(&app(), "A-001", &["R-999"], 20, "10:00", "11:00").await;

    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.json["error"], "room_not_found");
}

#[tokio::test]
async fn incomplete_body_is_422_in_api_error_format() {
    let reply = call(&app(), Method::POST, "/bookings", Some("A-001"), Some(r#"{"room_ids":["R-101"]}"#.into())).await;

    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.json["error"], "invalid_body");
}

#[tokio::test]
async fn unknown_field_is_rejected() {
    // Empêche par exemple d'envoyer une capacité ou un statut : ils viennent du serveur.
    let body = json!({ "room_ids": ["R-101"], "participants": 20, "event_date": event_date(), "event_start": "10:00", "event_end": "11:00", "status": "approved" });

    let reply = call(&app(), Method::POST, "/bookings", Some("A-001"), Some(body.to_string())).await;

    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.json["error"], "invalid_body");
}

#[tokio::test]
async fn malformed_json_is_400() {
    let reply = call(&app(), Method::POST, "/bookings", Some("A-001"), Some("{".into())).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.json["error"], "invalid_body");
}

// ---------- validation par le responsable ----------

#[tokio::test]
async fn amphi_request_goes_through_validation() {
    let state = app();
    let created = submit(&state, "A-001", &["R-001"], 80, "14:00", "16:00").await;
    let id = created.json["booking"]["id"].as_str().unwrap().to_string();
    let validate = format!("/bookings/{id}/validate");

    let by_applicant = call(&state, Method::POST, &validate, Some("A-001"), None).await;
    assert_eq!(by_applicant.status, StatusCode::FORBIDDEN);
    assert_eq!(by_applicant.json["error"], "missing_role");

    let by_responsible = call(&state, Method::POST, &validate, Some("RESP-001"), None).await;
    assert_eq!(by_responsible.status, StatusCode::OK);
    assert_eq!(by_responsible.json["status"], "approved");

    let again = call(&state, Method::POST, &validate, Some("RESP-001"), None).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json["error"], "booking_already_accepted");
}

#[tokio::test]
async fn responsible_can_decline_a_request() {
    let state = app_at(requested_at(), vec![booking("B-001", amphi(), BookingStatus::Requested, t(10, 0), t(12, 0))]);

    let reply = call(&state, Method::POST, "/bookings/B-001/decline", Some("RESP-001"), None).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json["status"], "declined");
    assert_eq!(state.notifier.sent(), vec![Notification::Declined("B-001".into())]);
}

// ---------- consultation ----------

#[tokio::test]
async fn booking_is_visible_to_owner_and_responsible_only() {
    let state = app_at(requested_at(), vec![approved_booking()]);

    assert_eq!(call(&state, Method::GET, "/bookings/B-001", Some("A-001"), None).await.status, StatusCode::OK);
    assert_eq!(call(&state, Method::GET, "/bookings/B-001", Some("RESP-001"), None).await.status, StatusCode::OK);
    let other = call(&state, Method::GET, "/bookings/B-001", Some("A-002"), None).await;
    assert_eq!(other.status, StatusCode::FORBIDDEN);
    assert_eq!(other.json["error"], "not_booking_owner");
}

#[tokio::test]
async fn unknown_booking_is_404() {
    let reply = call(&app(), Method::GET, "/bookings/B-404", Some("A-001"), None).await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.json["error"], "booking_not_found");
}

// ---------- annulation et check-in ----------

#[tokio::test]
async fn owner_can_cancel_more_than_2h_before() {
    let state = app_at(event_date().and_time(t(7, 0)), vec![approved_booking()]);

    let other = call(&state, Method::POST, "/bookings/B-001/cancel", Some("A-002"), None).await;
    assert_eq!(other.status, StatusCode::FORBIDDEN);

    let owner = call(&state, Method::POST, "/bookings/B-001/cancel", Some("A-001"), None).await;
    assert_eq!(owner.status, StatusCode::OK);
    assert_eq!(owner.json["status"], "cancelled");
}

#[tokio::test]
async fn cancellation_less_than_2h_before_is_409() {
    let state = app_at(event_date().and_time(t(9, 0)), vec![approved_booking()]);

    let reply = call(&state, Method::POST, "/bookings/B-001/cancel", Some("A-001"), None).await;

    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.json["error"], "cancellation_too_late");
}

#[tokio::test]
async fn check_in_within_15_min_is_accepted() {
    let state = app_at(event_date().and_time(t(10, 5)), vec![approved_booking()]);

    let reply = call(&state, Method::POST, "/bookings/B-001/check-in", Some("A-001"), None).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json["checked_in"], true);
}

#[tokio::test]
async fn late_check_in_is_409_and_booking_becomes_no_show() {
    let state = app_at(event_date().and_time(t(10, 30)), vec![approved_booking()]);

    let reply = call(&state, Method::POST, "/bookings/B-001/check-in", Some("A-001"), None).await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
    assert_eq!(reply.json["error"], "check_in_too_late");

    let after = call(&state, Method::GET, "/bookings/B-001", Some("A-001"), None).await;
    assert_eq!(after.json["status"], "no_show");
}

// ---------- panne ----------

#[tokio::test]
async fn storage_failure_is_500_without_leaking_details() {
    let state = Arc::new(AppState::<BrokenStorage>::new(
        users(),
        rooms(),
        FailingRepository,
        FakeNotifier::default(),
        FixedClock(requested_at()),
    ));

    let reply = submit(&state, "A-001", &["R-101"], 20, "10:00", "11:00").await;

    assert_eq!(reply.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(reply.json["error"], "internal_error");
    assert!(!reply.json["message"].as_str().unwrap().contains("base de données"));
    assert!(state.notifier.sent().is_empty());
}
