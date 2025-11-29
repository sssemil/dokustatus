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
            AppError::Database(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
            }
            AppError::InvalidCredentials => {
                (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response()
            }
            AppError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({"message": "Too many requests. Please try again later."})),
            )
                .into_response(),
            AppError::InvalidInput(msg) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "message": msg })),
            )
                .into_response(),
            AppError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
            }
        }
    }
}
