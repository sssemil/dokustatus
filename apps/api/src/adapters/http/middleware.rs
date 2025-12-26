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
    // Only trust forwarded headers if explicitly configured (when behind a reverse proxy)
    let ip = if app_state.config.trust_proxy {
        forwarded_ip(&request).unwrap_or_else(|| addr.ip().to_string())
    } else {
        addr.ip().to_string()
    };
    let email = cookies.get("user_email").map(|c| c.value().to_owned());

    tracing::debug!(
        trust_proxy = app_state.config.trust_proxy,
        connect_ip = %addr.ip(),
        forwarded_ip = ?forwarded_ip(&request),
        using_ip = %ip,
        email = ?email,
        "Rate limiting request"
    );

    app_state.rate_limiter.check(&ip, email.as_deref()).await?;

    // Preserve cookie jar for downstream extractors.
    request.extensions_mut().insert(cookies);

    Ok(next.run(request).await)
}

fn forwarded_ip(req: &Request) -> Option<String> {
    // Extract IP from X-Forwarded-For or X-Real-IP headers
    if let Some(forwarded) = req.headers().get("x-forwarded-for")
        && let Ok(val) = forwarded.to_str()
        && let Some(first) = val.split(',').next()
    {
        let trimmed = first.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(real) = req.headers().get("x-real-ip")
        && let Ok(val) = real.to_str()
        && !val.trim().is_empty()
    {
        return Some(val.trim().to_string());
    }
    None
}
