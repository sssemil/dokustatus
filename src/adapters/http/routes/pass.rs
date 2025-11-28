use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    adapters::http::app_state::AppState,
    app_error::{AppError, AppResult},
    application::jwt,
    use_cases::pass_status::{DocumentTrack, PassStatusMetadata, StatusCheckResult},
};

#[derive(Deserialize)]
struct AddPayload {
    number: String,
}

#[derive(Serialize)]
struct TracksResponse<T> {
    items: Vec<T>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/documents", get(list_tracks).post(add_track))
        .route("/documents/{id}", axum::routing::delete(delete_track))
        .route("/documents/check", post(check_tracks))
        .route("/documents/info", get(info))
}

async fn add_track(
    State(app_state): State<AppState>,
    cookies: CookieJar,
    Json(payload): Json<AddPayload>,
) -> AppResult<impl IntoResponse> {
    let user_id = current_user_id(&cookies, &app_state)?;

    let track = app_state
        .pass_status_use_cases
        .add_track(user_id, &payload.number)
        .await?;

    Ok((StatusCode::CREATED, Json(track)))
}

async fn list_tracks(
    State(app_state): State<AppState>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    let user_id = current_user_id(&cookies, &app_state)?;
    let tracks: Vec<DocumentTrack> = app_state.pass_status_use_cases.list_tracks(user_id).await?;
    Ok(Json(TracksResponse { items: tracks }))
}

async fn check_tracks(
    State(app_state): State<AppState>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    let user_id = current_user_id(&cookies, &app_state)?;
    let statuses: Vec<StatusCheckResult> = app_state
        .pass_status_use_cases
        .check_and_notify(user_id)
        .await?;
    Ok(Json(TracksResponse { items: statuses }))
}

async fn delete_track(
    State(app_state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let user_id = current_user_id(&cookies, &app_state)?;
    app_state
        .pass_status_use_cases
        .delete_track(user_id, id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn info(State(app_state): State<AppState>) -> AppResult<impl IntoResponse> {
    let meta: PassStatusMetadata = app_state.pass_status_use_cases.metadata().await?;
    Ok(Json(meta))
}

fn current_user_id(cookies: &CookieJar, app_state: &AppState) -> AppResult<Uuid> {
    let access_token = cookies
        .get("access_token")
        .ok_or(AppError::InvalidCredentials)?;
    let claims = jwt::verify(access_token.value(), &app_state.config.jwt_secret)?;
    Uuid::parse_str(&claims.sub).map_err(|_| AppError::InvalidCredentials)
}
