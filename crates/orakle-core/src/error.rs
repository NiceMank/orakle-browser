use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrakleError {
    #[error("Erreur de stockage SQLite : {0}")]
    Storage(#[from] rusqlite::Error),

    #[error("Erreur d'E/S système : {0}")]
    Io(#[from] std::io::Error),

    #[error("Erreur d'URL invalide : {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("Erreur TLS / Cryptographie : {0}")]
    Tls(String),

    #[error("Erreur réseau : {0}")]
    Network(String),

    #[error("Erreur de filtrage : {0}")]
    Shield(String),

    #[error("Erreur de proxy : {0}")]
    Proxy(String),

    #[error("Tentative d'accès non autorisée / SSRF bloquée : {0}")]
    SecurityViolation(String),
}

pub type Result<T> = std::result::Result<T, OrakleError>;
