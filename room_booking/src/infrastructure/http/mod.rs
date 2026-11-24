//! API HTTP (axum), adaptateur « entrant » : traduit les requêtes en appels de use cases.
//!
//! | Méthode | Chemin                     | Qui              |
//! |---------|----------------------------|------------------|
//! | GET     | /rooms                     | public           |
//! | GET     | /rooms/{id}/availability   | public           |
//! | GET     | /users                     | public (provisoire) |
//! | GET     | /bookings?status=…         | responsable      |
//! | GET     | /bookings/mine             | tout utilisateur |
//! | POST    | /bookings                  | demandeur        |
//! | GET     | /bookings/{id}             | demandeur, resp. |
//! | POST    | /bookings/{id}/validate    | responsable      |
//! | POST    | /bookings/{id}/decline     | responsable      |
//! | POST    | /bookings/{id}/cancel      | demandeur        |
//! | POST    | /bookings/{id}/check-in    | demandeur        |
//!
//! L'appelant s'identifie avec l'en-tête `X-User-Id` (voir `auth`).
//! Le CORS (appels depuis le serveur de dev du frontend) est configuré dans `cors`.
//!
//! Documentation interactive : Swagger UI sur `/docs` (spec OpenAPI sur `/api-docs/openapi.json`),
//! générée depuis les attributs `#[utoipa::path]` des handlers (voir `openapi`).

mod auth;
mod cors;
mod dto;
mod error;
mod handlers;
mod openapi;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub use auth::USER_ID_HEADER;
pub use cors::{cors_layer, parse_origins};
pub use openapi::{ApiDoc, DOCS_PATH, SPEC_PATH};

use crate::infrastructure::state::{Adapters, AppState};

pub fn router<A: Adapters>(state: Arc<AppState<A>>) -> Router {
    Router::new()
        .route("/rooms", get(handlers::list_rooms::<A>))
        .route("/rooms/{id}/availability", get(handlers::room_availability_handler::<A>))
        .route("/users", get(handlers::list_users::<A>))
        .route("/bookings", get(handlers::list_all::<A>).post(handlers::submit::<A>))
        // Chemin fixe : prioritaire sur `/bookings/{id}` (les ids sont des UUID, jamais « mine »).
        .route("/bookings/mine", get(handlers::list_mine::<A>))
        .route("/bookings/{id}", get(handlers::show::<A>))
        .route("/bookings/{id}/validate", post(handlers::validate::<A>))
        .route("/bookings/{id}/decline", post(handlers::decline::<A>))
        .route("/bookings/{id}/cancel", post(handlers::cancel::<A>))
        .route("/bookings/{id}/check-in", post(handlers::check_in::<A>))
        .with_state(state)
        .merge(SwaggerUi::new(DOCS_PATH).url(SPEC_PATH, ApiDoc::openapi()))
}
