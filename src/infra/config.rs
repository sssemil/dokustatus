use std::net::SocketAddr;

use axum::http::HeaderValue;
use env_helpers::{get_env, get_env_default};
use secrecy::SecretString;
use time::Duration;
use url::Url;

pub struct AppConfig {
    pub jwt_secret: SecretString,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
    pub resend_api_key: SecretString,
    pub email_from: String,
    pub app_origin: Url,
    pub cors_origin: HeaderValue,
    pub magic_link_ttl_minutes: i64,
    pub bind_addr: SocketAddr,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let jwt_secret: SecretString = SecretString::new(get_env::<String>("JWT_SECRET").into());

        let refresh_token_ttl_days: i64 = get_env_default("REFRESH_TOKEN_TTL_DAYS", 30);

        let access_token_ttl_secs: i64 = get_env_default("ACCESS_TOKEN_TTL_SECS", 30);

        let resend_api_key: SecretString =
            SecretString::new(get_env::<String>("RESEND_API_KEY").into());
        let email_from: String = get_env("EMAIL_FROM");
        let app_origin: Url = get_env("APP_ORIGIN");
        let magic_link_ttl_minutes: i64 = get_env_default("MAGIC_LINK_TTL_MINUTES", 15);
        let cors_origin: HeaderValue =
            get_env_default("CORS_ORIGIN", String::from("http://localhost:3000"))
                .parse()
                .expect("CORS_ORIGIN must be a valid header value");

        let bind_addr: SocketAddr = get_env_default("BIND_ADDR", "127.0.0.1:3001".parse().unwrap());

        Self {
            jwt_secret,
            access_token_ttl: Duration::seconds(access_token_ttl_secs),
            refresh_token_ttl: Duration::days(refresh_token_ttl_days),
            resend_api_key,
            email_from,
            app_origin,
            magic_link_ttl_minutes,
            cors_origin,
            bind_addr,
        }
    }
}
