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
                error_resp(StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::DatabaseError, None)
            }
            AppError::InvalidCredentials => {
                error_resp(StatusCode::UNAUTHORIZED, ErrorCode::InvalidCredentials, None)
            }
            AppError::InvalidApiKey => {
                error_resp(StatusCode::UNAUTHORIZED, ErrorCode::InvalidApiKey, Some("Invalid or missing API key".to_string()))
            }
            AppError::RateLimited => {
                error_resp(StatusCode::TOO_MANY_REQUESTS, ErrorCode::RateLimited, None)
            }
            AppError::InvalidInput(msg) => {
                error_resp(StatusCode::BAD_REQUEST, ErrorCode::InvalidInput, Some(msg))
            }
            AppError::AccountSuspended => {
                error_resp(StatusCode::FORBIDDEN, ErrorCode::AccountSuspended, Some("Your account has been suspended".to_string()))
            }
            AppError::TooManyDocuments => {
                error_resp(StatusCode::BAD_REQUEST, ErrorCode::TooManyDocuments, None)
            }
            AppError::NotFound => {
                error_resp(StatusCode::NOT_FOUND, ErrorCode::NotFound, None)
            }
            AppError::SessionMismatch => {
                error_resp(StatusCode::UNAUTHORIZED, ErrorCode::SessionMismatch, Some("Please use the same browser or device where you requested the login link".to_string()))
            }
            AppError::Internal(_) => {
                error_resp(StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::InternalError, None)
            }
        }
    }
}

fn error_resp(status: StatusCode, code: ErrorCode, message: Option<String>) -> Response {
    let body = match message {
        Some(msg) => serde_json::json!({ "code": code.as_str(), "message": msg }),
        None => serde_json::json!({ "code": code.as_str() }),
    };
    (status, Json(body)).into_response()
}
