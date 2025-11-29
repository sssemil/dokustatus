use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;

use crate::{adapters::http::app_state::AppState, app_error::AppError};

pub async fn rate_limit_middleware(
    State(app_state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    cookies: CookieJar,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let ip = addr.ip().to_string();
    let email = cookies.get("user_email").map(|c| c.value().to_owned());

    app_state.rate_limiter.check(&ip, email.as_deref()).await?;

    // Preserve cookie jar for downstream extractors.
    request.extensions_mut().insert(cookies);

    Ok(next.run(request).await)
}
