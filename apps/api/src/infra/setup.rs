use crate::{
    adapters::{
        email::resend::ResendEmailSender, http::app_state::AppState,
        pass_status::MunichPassStatusClient,
    },
    infra::{
        config::AppConfig, crypto::ProcessCipher, magic_links::MagicLinkStore,
        postgres_persistence, rate_limit::RateLimiter,
    },
    use_cases::{
        pass_status::{PassStatusRepo, PassStatusUseCases},
        user::{AuthUseCases, UserRepo},
    },
};
use std::fs::File;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub async fn init_app_state() -> anyhow::Result<AppState> {
    let config = AppConfig::from_env();

    let cipher = Arc::new(ProcessCipher::new_from_base64(&config.process_number_key)?);
    let postgres_arc = Arc::new(postgres_persistence(cipher.clone(), &config.database_url).await?);

    let rate_limiter = Arc::new(
        RateLimiter::new(
            &config.redis_url,
            config.rate_limit_window_secs,
            config.rate_limit_per_ip,
            config.rate_limit_per_email,
        )
        .await?,
    );

    let magic_links = Arc::new(MagicLinkStore::new(&config.redis_url).await?);

    let email = Arc::new(ResendEmailSender::new(
        config.resend_api_key.clone(),
        config.email_from.clone(),
    ));

    let user_repo_arc = postgres_arc.clone() as Arc<dyn UserRepo>;
    let pass_repo_arc = postgres_arc.clone() as Arc<dyn PassStatusRepo>;
    let status_client = Arc::new(MunichPassStatusClient::new(
        config.pass_status_url.to_string(),
        config.pass_status_info_url.to_string(),
    ));

    let auth_use_cases = AuthUseCases::new(
        user_repo_arc.clone(),
        magic_links,
        email.clone(),
        config.app_origin.to_string(),
    );

    let pass_status_use_cases = PassStatusUseCases::new(
        pass_repo_arc.clone(),
        email,
        status_client,
        user_repo_arc.clone(),
        config.pass_status_poll_seconds,
        config.max_documents_per_user,
        config.app_origin.to_string(),
    );

    Ok(AppState {
        config: Arc::new(config),
        auth_use_cases: Arc::new(auth_use_cases),
        pass_status_use_cases: Arc::new(pass_status_use_cases),
        user_repo: user_repo_arc,
        rate_limiter,
    })
}

pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "axum_trainer=debug,tower_http=debug".into());

    // Console (pretty logs)
    let console_layer = fmt::layer()
        .with_target(false) // don’t show target (module path)
        .with_level(true) // show log level
        .pretty(); // human-friendly, with colors

    // File (structured JSON logs)
    let file = File::create("app.log").expect("cannot create log file");
    let json_layer = fmt::layer()
        .json()
        .with_writer(file)
        .with_current_span(true)
        .with_span_list(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(json_layer)
        .try_init()
        .ok();
}
