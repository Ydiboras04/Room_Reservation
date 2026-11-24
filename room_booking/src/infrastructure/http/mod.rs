//! API HTTP (axum), adaptateur « entrant » : traduit les requêtes en appels de use cases.
//!
//! | Méthode | Chemin                     | Qui              |
//! |---------|----------------------------|------------------|
//! | GET     | /rooms                     | public           |
//! | POST    | /bookings                  | demandeur        |
//! | GET     | /bookings/{id}             | demandeur, resp. |
//! | POST    | /bookings/{id}/validate    | responsable      |
//! | POST    | /bookings/{id}/decline     | responsable      |
//! | POST    | /bookings/{id}/cancel      | demandeur        |
//! | POST    | /bookings/{id}/check-in    | demandeur        |
//!
//! L'appelant s'identifie avec l'en-tête `X-User-Id` (voir `auth`).

mod auth;
mod dto;
mod error;
mod handlers;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

pub use auth::USER_ID_HEADER;

use crate::infrastructure::state::{Adapters, AppState};

pub fn router<A: Adapters>(state: Arc<AppState<A>>) -> Router {
    Router::new()
        .route("/rooms", get(handlers::list_rooms::<A>))
        .route("/bookings", post(handlers::submit::<A>))
        .route("/bookings/{id}", get(handlers::show::<A>))
        .route("/bookings/{id}/validate", post(handlers::validate::<A>))
        .route("/bookings/{id}/decline", post(handlers::decline::<A>))
        .route("/bookings/{id}/cancel", post(handlers::cancel::<A>))
        .route("/bookings/{id}/check-in", post(handlers::check_in::<A>))
        .with_state(state)
}
