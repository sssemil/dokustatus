use axum::{Json, Router, extract::Path, http::StatusCode, response::IntoResponse, routing::get};

use crate::{
    adapters::http::app_state::AppState,
    application::{dictionaries::dictionary_json, language::UserLanguage},
};

pub fn router() -> Router<AppState> {
    Router::new().route("/{lang}", get(get_dictionary))
}

async fn get_dictionary(Path(lang): Path<String>) -> impl IntoResponse {
    let lang = match lang.as_str() {
        "en" => UserLanguage::En,
        "de" => UserLanguage::De,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let dict = dictionary_json(lang);
    Json(dict).into_response()
}
