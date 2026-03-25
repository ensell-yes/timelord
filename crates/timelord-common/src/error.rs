use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden")]
    Forbidden,

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl AppError {
    pub fn internal(msg: impl std::fmt::Display) -> Self {
        Self::Internal(msg.to_string())
    }

    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            AppError::Internal(_) | AppError::Anyhow(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR")
            }
            AppError::Database(e) => {
                if let sqlx::Error::RowNotFound = e {
                    (StatusCode::NOT_FOUND, "NOT_FOUND")
                } else {
                    (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR")
                }
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();

        // Don't leak internal error details to clients
        let message = match &self {
            AppError::Internal(_) | AppError::Anyhow(_) | AppError::Database(_) => {
                tracing::error!(error = %self, "internal error");
                "An internal error occurred".to_string()
            }
            other => other.to_string(),
        };

        let body = Json(json!({
            "error": {
                "code": code,
                "message": message,
            }
        }));

        (status, body).into_response()
    }
}

impl From<AppError> for tonic::Status {
    fn from(err: AppError) -> Self {
        match err {
            AppError::NotFound(msg) => tonic::Status::not_found(msg),
            AppError::Unauthorized => tonic::Status::unauthenticated("Unauthorized"),
            AppError::Forbidden => tonic::Status::permission_denied("Forbidden"),
            AppError::BadRequest(msg) => tonic::Status::invalid_argument(msg),
            AppError::Conflict(msg) => tonic::Status::already_exists(msg),
            AppError::Internal(msg) => tonic::Status::internal(msg),
            AppError::Database(e) => tonic::Status::internal(e.to_string()),
            AppError::Anyhow(e) => tonic::Status::internal(e.to_string()),
        }
    }
}
