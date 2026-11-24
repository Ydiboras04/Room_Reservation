//! CORS : autorise le frontend, servi depuis une autre origine (ex. Vite sur le port 5173),
//! à appeler l'API depuis le navigateur.
//!
//! Seules les origines listées sont acceptées (pas de `*`) : une page d'un autre site
//! ne peut pas appeler l'API au nom de l'utilisateur.

use std::time::Duration;

use axum::http::{
    HeaderName, HeaderValue, Method,
    header::{CONTENT_TYPE, LOCATION},
};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::infrastructure::http::USER_ID_HEADER;

pub fn cors_layer(allowed_origins: Vec<HeaderValue>) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([CONTENT_TYPE, HeaderName::from_static(USER_ID_HEADER)])
        // Sans cela, le navigateur cache l'en-tête Location de `POST /bookings` au JavaScript.
        .expose_headers([LOCATION])
        // Le navigateur garde la réponse du preflight 1 h au lieu de la redemander à chaque appel.
        .max_age(Duration::from_secs(3600))
}

/// Lit une liste d'origines séparées par des virgules : `"http://localhost:5173, http://127.0.0.1:5173"`.
pub fn parse_origins(raw: &str) -> Result<Vec<HeaderValue>, String> {
    raw.split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            if !(origin.starts_with("http://") || origin.starts_with("https://")) || origin.ends_with('/') {
                return Err(format!("origine CORS invalide « {origin} » : attendu par ex. http://localhost:5173"));
            }
            HeaderValue::from_str(origin).map_err(|_| format!("origine CORS invalide « {origin} »"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_origins;

    #[test]
    fn parses_a_comma_separated_list() {
        let origins = parse_origins(" http://localhost:5173 ,https://app.example.com,").unwrap();

        assert_eq!(origins, vec!["http://localhost:5173", "https://app.example.com"]);
    }

    #[test]
    fn rejects_origins_without_scheme_or_with_trailing_slash() {
        assert!(parse_origins("localhost:5173").is_err());
        assert!(parse_origins("http://localhost:5173/").is_err());
    }
}
