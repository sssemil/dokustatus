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
    pub app_origin: Url,
    pub cors_origin: HeaderValue,
    pub magic_link_ttl_minutes: i64,
    pub bind_addr: SocketAddr,
    pub redis_url: String,
    pub rate_limit_window_secs: u64,
    pub rate_limit_per_ip: u64,
    pub rate_limit_per_email: u64,
    pub database_url: String,
    /// Whether to trust X-Forwarded-For headers. Set to true when behind a reverse proxy (Caddy, nginx).
    /// SECURITY: Only enable this when the API is not directly exposed to the internet.
    pub trust_proxy: bool,
    /// The ingress domain that custom domains should CNAME to (e.g., "ingress.reauth.dev").
    pub ingress_domain: String,
    /// Optional DNS server address for lookups (e.g., "127.0.0.1:5353" for local CoreDNS).
    pub dns_server: Option<SocketAddr>,
    /// The main reauth domain (e.g., "reauth.dev" in prod, "reauth.test" for local dev).
    pub main_domain: String,
    /// Fallback Resend API key for domains without custom email config.
    pub fallback_resend_api_key: String,
    /// Fallback email domain (e.g., "hi.reauth.dev"). Used with domain name to construct from email.
    pub fallback_email_domain: String,
    /// Fallback Google OAuth client ID for domains without custom OAuth config.
    pub fallback_google_client_id: String,
    /// Fallback Google OAuth client secret for domains without custom OAuth config.
    pub fallback_google_client_secret: String,
    // NOTE: No fallback Stripe credentials - we cannot accept payments on behalf of other developers.
    // Each domain must configure their own Stripe account.
}

impl AppConfig {
    pub fn from_env() -> Self {
        let jwt_secret: SecretString = SecretString::new(get_env::<String>("JWT_SECRET").into());

        let refresh_token_ttl_days: i64 = get_env_default("REFRESH_TOKEN_TTL_DAYS", 30);

        let access_token_ttl_secs: i64 = get_env_default("ACCESS_TOKEN_TTL_SECS", 86_400);

        let app_origin: Url = get_env("APP_ORIGIN");
        let magic_link_ttl_minutes: i64 = get_env_default("MAGIC_LINK_TTL_MINUTES", 15);
        let cors_origin: HeaderValue =
            get_env_default("CORS_ORIGIN", String::from("http://localhost:3000"))
                .parse()
                .expect("CORS_ORIGIN must be a valid header value");

        let bind_addr: SocketAddr = get_env_default("BIND_ADDR", "127.0.0.1:3001".parse().unwrap());
        let redis_url: String = get_env_default("REDIS_URL", "redis://127.0.0.1:6379".to_string());
        let rate_limit_window_secs: u64 = get_env_default("RATE_LIMIT_WINDOW_SECS", 60);
        let rate_limit_per_ip: u64 = get_env_default("RATE_LIMIT_PER_IP", 60);
        let rate_limit_per_email: u64 = get_env_default("RATE_LIMIT_PER_EMAIL", 30);
        let database_url: String = get_env("DATABASE_URL");
        // Default to false for security - must explicitly enable when behind a trusted proxy
        let trust_proxy: bool = get_env_default("TRUST_PROXY", false);
        let ingress_domain: String =
            get_env_default("INGRESS_DOMAIN", "ingress.reauth.dev".to_string());
        let dns_server: Option<SocketAddr> = std::env::var("DNS_SERVER")
            .ok()
            .and_then(|s| s.parse().ok());
        let main_domain: String = get_env_default("MAIN_DOMAIN", "reauth.dev".to_string());
        let fallback_resend_api_key: String = get_env("FALLBACK_RESEND_API_KEY");
        let fallback_email_domain: String = get_env("FALLBACK_EMAIL_DOMAIN");
        let fallback_google_client_id: String = get_env("FALLBACK_GOOGLE_CLIENT_ID");
        let fallback_google_client_secret: String = get_env("FALLBACK_GOOGLE_CLIENT_SECRET");
        // NOTE: No fallback Stripe credentials - each domain must configure their own Stripe account.

        Self {
            jwt_secret,
            access_token_ttl: Duration::seconds(access_token_ttl_secs),
            refresh_token_ttl: Duration::days(refresh_token_ttl_days),
            app_origin,
            magic_link_ttl_minutes,
            cors_origin,
            bind_addr,
            redis_url,
            rate_limit_window_secs,
            rate_limit_per_ip,
            rate_limit_per_email,
            database_url,
            trust_proxy,
            ingress_domain,
            dns_server,
            main_domain,
            fallback_resend_api_key,
            fallback_email_domain,
            fallback_google_client_id,
            fallback_google_client_secret,
        }
    }
}
