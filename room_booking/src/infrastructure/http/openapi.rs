//! Documentation OpenAPI, générée depuis le code :
//! - chemins et réponses : attributs `#[utoipa::path]` des handlers ;
//! - schémas JSON : `ToSchema` / `IntoParams` des DTO.
//!
//! Servie par l'API : Swagger UI sur `/docs`, spec brute sur `/api-docs/openapi.json`
//! (utilisable pour générer un client TypeScript côté frontend).

use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
};

use crate::infrastructure::http::{USER_ID_HEADER, handlers};

pub const DOCS_PATH: &str = "/docs";
pub const SPEC_PATH: &str = "/api-docs/openapi.json";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Room Booking API",
        description = "API de réservation de salles.\n\n\
            **Identification** : pas encore d'authentification, l'appelant indique son id dans l'en-tête \
            `X-User-Id`. Dans Swagger UI, cliquez sur **Authorize** et saisissez par exemple `A-001` \
            (demandeur), `A-002` (demandeur) ou `RESP-001` (responsable).\n\n\
            **Erreurs** : corps `{\"error\": \"<code>\", \"message\": \"...\"}`. Le code est stable, \
            le message peut changer : les clients doivent tester `error`.",
    ),
    paths(
        handlers::list_rooms,
        handlers::room_availability_handler,
        handlers::list_users,
        handlers::list_mine,
        handlers::list_all,
        handlers::submit,
        handlers::show,
        handlers::validate,
        handlers::decline,
        handlers::cancel,
        handlers::check_in,
    ),
    modifiers(&UserIdHeader),
    tags(
        (name = "Salles", description = "Catalogue et disponibilités (public)"),
        (name = "Réservations", description = "Cycle de vie d'une réservation"),
        (name = "Utilisateurs", description = "Provisoire, en attendant l'authentification"),
    ),
)]
pub struct ApiDoc;

/// Déclare l'en-tête `X-User-Id` comme schéma de sécurité « user_id »
/// (référencé par `security(("user_id" = []))` dans les handlers).
struct UserIdHeader;

impl Modify for UserIdHeader {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "user_id",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                "X-User-Id",
                "Id de l'utilisateur qui fait la requête (ex. A-001, RESP-001).",
            ))),
        );
        // Garde-fou : le nom déclaré ici doit rester celui que lit l'extracteur.
        debug_assert!(USER_ID_HEADER.eq_ignore_ascii_case("X-User-Id"));
    }
}
