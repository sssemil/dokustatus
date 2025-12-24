use crate::app_error::{AppError, ErrorCode};
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
                error_resp(StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::DatabaseError)
            }
            AppError::InvalidCredentials => {
                error_resp(StatusCode::UNAUTHORIZED, ErrorCode::InvalidCredentials)
            }
            AppError::RateLimited => {
                error_resp(StatusCode::TOO_MANY_REQUESTS, ErrorCode::RateLimited)
            }
            AppError::InvalidInput(_) => {
                error_resp(StatusCode::BAD_REQUEST, ErrorCode::InvalidInput)
            }
            AppError::TooManyDocuments => {
                error_resp(StatusCode::BAD_REQUEST, ErrorCode::TooManyDocuments)
            }
            AppError::NotFound => {
                error_resp(StatusCode::NOT_FOUND, ErrorCode::NotFound)
            }
            AppError::Internal(_) => {
                error_resp(StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::InternalError)
            }
        }
    }
}

fn error_resp(status: StatusCode, code: ErrorCode) -> Response {
    (status, Json(serde_json::json!({ "code": code.as_str() }))).into_response()
}
