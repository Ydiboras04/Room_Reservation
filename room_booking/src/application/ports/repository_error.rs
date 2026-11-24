use std::error::Error;

/// Échec technique d'un dépôt (connexion perdue, contrainte violée, délai dépassé…).
///
/// « Rien trouvé » n'est pas une erreur : c'est `Ok(None)`. Cette erreur est réservée
/// aux cas où le stockage n'a pas pu répondre. L'adaptateur y range l'erreur d'origine
/// (sqlx, I/O…) pour qu'elle reste accessible via `source()`.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("erreur du stockage : {0}")]
    Storage(#[source] Box<dyn Error + Send + Sync>),
}

impl RepositoryError {
    pub fn storage(error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self::Storage(error.into())
    }
}
