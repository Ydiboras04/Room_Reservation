//! Identification de l'appelant.
//!
//! Pas encore d'authentification : l'appelant déclare son id dans l'en-tête `X-User-Id`.
//! C'est volontairement isolé ici : le jour où un vrai jeton (JWT, session…) arrive,
//! seul cet extracteur change, les handlers continuent de recevoir un `CurrentUser`.

use axum::{extract::FromRequestParts, http::{StatusCode, request::Parts}};

use crate::infrastructure::http::error::ApiError;

pub const USER_ID_HEADER: &str = "x-user-id";

/// Id de l'utilisateur qui fait la requête (pas encore vérifié : les use cases chargent
/// l'utilisateur et renvoient « inconnu » s'il n'existe pas).
pub struct CurrentUser(pub String);

impl<S: Send + Sync> FromRequestParts<S> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .headers
            .get(USER_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| CurrentUser(id.to_string()))
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::UNAUTHORIZED,
                    "missing_user",
                    "en-tête X-User-Id manquant : indiquez l'id de l'utilisateur",
                )
            })
    }
}
