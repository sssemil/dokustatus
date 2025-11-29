use crate::app_error::AppError;
use axum::Json;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Log the error before it gets converted into a status response.
        tracing::error!(error = ?self, "Request failed");

        match self {
            AppError::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "code": "DATABASE_ERROR" })),
            )
                .into_response(),
            AppError::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "code": "INVALID_CREDENTIALS" })),
            )
                .into_response(),
            AppError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({"code": "RATE_LIMITED"})),
            )
                .into_response(),
            AppError::InvalidInput(_) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "code": "INVALID_INPUT" })),
            )
                .into_response(),
            AppError::TooManyDocuments => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "code": "TOO_MANY_DOCUMENTS" })),
            )
                .into_response(),
            AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "code": "INTERNAL_ERROR" })),
            )
                .into_response(),
        }
    }
}
