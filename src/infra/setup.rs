use crate::{
    adapters::{
        email::resend::ResendEmailSender,
        http::app_state::AppState,
    },
    infra::{config::AppConfig, postgres_persistence},
    use_cases::user::{AuthUseCases, UserRepo},
};
use std::fs::File;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub async fn init_app_state() -> anyhow::Result<AppState> {
    let config = AppConfig::from_env();

    let postgres_arc = Arc::new(postgres_persistence().await?);

    let email = Arc::new(ResendEmailSender::new(
        config.resend_api_key.clone(),
        config.email_from.clone(),
    ));

    let auth_use_cases = AuthUseCases::new(
        postgres_arc.clone() as Arc<dyn UserRepo>,
        email,
        config.app_origin.to_string(),
    );

    Ok(AppState { config: Arc::new(config), auth_use_cases: Arc::new(auth_use_cases) })
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
