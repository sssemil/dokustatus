use std::env;

use time::Duration;

pub struct AppConfig {
    pub jwt_secret: String,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
    pub resend_api_key: String,
    pub email_from: String,
    pub app_origin: String,
    pub magic_link_ttl_minutes: i64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");

        let refresh_token_ttl_days: i64 = env::var("REFRESH_TOKEN_TTL_DAYS")
            .unwrap_or("30".to_string())
            .parse()
            .expect("REFRESH_TOKEN_TTL_DAYS must be a valid number");

        let access_token_ttl_secs: i64 = env::var("ACCESS_TOKEN_TTL_SECS")
            .unwrap_or("30".to_string())
            .parse()
            .expect("ACCESS_TOKEN_TTL_SECS must be a valid number");

        let resend_api_key = env::var("RESEND_API_KEY").expect("RESEND_API_KEY must be set");
        let email_from = env::var("EMAIL_FROM").expect("EMAIL_FROM must be set");
        let app_origin = env::var("APP_ORIGIN").expect("APP_ORIGIN must be set");
        let magic_link_ttl_minutes: i64 = env::var("MAGIC_LINK_TTL_MINUTES")
            .unwrap_or("15".to_string())
            .parse()
            .expect("MAGIC_LINK_TTL_MINUTES must be a valid number");

        Self {
            jwt_secret,
            access_token_ttl: Duration::seconds(access_token_ttl_secs),
            refresh_token_ttl: Duration::days(refresh_token_ttl_days),
            resend_api_key,
            email_from,
            app_origin,
            magic_link_ttl_minutes,
        }
    }
}
