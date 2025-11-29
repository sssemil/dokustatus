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
    pub pass_status_url: Url,
    pub pass_status_poll_seconds: i64,
    pub pass_status_info_url: Url,
    pub pass_status_counter_url: Option<Url>,
    pub redis_url: String,
    pub rate_limit_window_secs: u64,
    pub rate_limit_per_ip: u64,
    pub rate_limit_per_email: u64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let jwt_secret: SecretString = SecretString::new(get_env::<String>("JWT_SECRET").into());

        let refresh_token_ttl_days: i64 = get_env_default("REFRESH_TOKEN_TTL_DAYS", 30);

        let access_token_ttl_secs: i64 = get_env_default("ACCESS_TOKEN_TTL_SECS", 86_400);

        let resend_api_key: SecretString =
            SecretString::new(get_env::<String>("RESEND_API_KEY").into());
        let email_from: String = get_env("EMAIL_FROM");
        let app_origin: Url = get_env("APP_ORIGIN");
        let magic_link_ttl_minutes: i64 = get_env_default("MAGIC_LINK_TTL_MINUTES", 15);
        let cors_origin: HeaderValue =
            get_env_default("CORS_ORIGIN", String::from("http://localhost:3000"))
                .parse()
                .expect("CORS_ORIGIN must be a valid header value");

        let pass_status_url: Url = get_env_default(
            "PASS_STATUS_URL",
            String::from(
                "https://mpdz-passverfolgung.muenchen.de/api/passstatusabfrage-backend-service/rest/ausweisstatus/search",
            ),
        )
        .parse()
        .expect("PASS_STATUS_URL must be a valid URL");
        let pass_status_info_url: Url = get_env_default(
            "PASS_STATUS_INFO_URL",
            String::from("https://mpdz-passverfolgung.muenchen.de/actuator/info"),
        )
        .parse()
        .expect("PASS_STATUS_INFO_URL must be a valid URL");
        let pass_status_counter_url: Option<Url> =
            std::env::var("PASS_STATUS_COUNTER_URL").ok().map(|v| {
                v.parse()
                    .expect("PASS_STATUS_COUNTER_URL must be a valid URL")
            });

        let bind_addr: SocketAddr = get_env_default("BIND_ADDR", "127.0.0.1:3001".parse().unwrap());
        let pass_status_poll_seconds: i64 = get_env_default("PASS_STATUS_POLL_SECONDS", 3600);
        let redis_url: String = get_env_default("REDIS_URL", "redis://127.0.0.1:6379".to_string());
        let rate_limit_window_secs: u64 = get_env_default("RATE_LIMIT_WINDOW_SECS", 60);
        let rate_limit_per_ip: u64 = get_env_default("RATE_LIMIT_PER_IP", 60);
        let rate_limit_per_email: u64 = get_env_default("RATE_LIMIT_PER_EMAIL", 30);

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
            pass_status_url,
            pass_status_poll_seconds,
            pass_status_info_url,
            pass_status_counter_url,
            redis_url,
            rate_limit_window_secs,
            rate_limit_per_ip,
            rate_limit_per_email,
        }
    }
}
