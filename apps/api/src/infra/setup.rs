use crate::{
    adapters::{dns::HickoryDnsVerifier, email::resend::ResendEmailSender, http::app_state::AppState},
    application::use_cases::domain_auth::{
        DomainAuthConfigRepo, DomainAuthMagicLinkRepo, DomainAuthUseCases,
        DomainEndUserRepo,
    },
    infra::{
        config::AppConfig,
        crypto::ProcessCipher,
        domain_email::DomainEmailSender,
        domain_magic_links::DomainMagicLinkStore,
        magic_links::MagicLinkStore,
        postgres_persistence,
        rate_limit::RateLimiter,
    },
    use_cases::domain::{DomainRepo, DomainUseCases},
    use_cases::user::{AuthUseCases, UserRepo},
};
use secrecy::ExposeSecret;
use std::fs::File;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub async fn init_app_state() -> anyhow::Result<AppState> {
    let config = AppConfig::from_env();

    let postgres_arc = Arc::new(postgres_persistence(&config.database_url).await?);

    let rate_limiter = Arc::new(
        RateLimiter::new(
            &config.redis_url,
            config.rate_limit_window_secs,
            config.rate_limit_per_ip,
            config.rate_limit_per_email,
        )
        .await?,
    );

    // Redis connection for magic links
    let redis_client = redis::Client::open(config.redis_url.as_str())?;
    let redis_manager = redis::aio::ConnectionManager::new(redis_client).await?;

    let magic_links = Arc::new(MagicLinkStore::new(&config.redis_url).await?);
    let domain_magic_links = Arc::new(DomainMagicLinkStore::new(redis_manager));

    let email = Arc::new(ResendEmailSender::new(
        config.resend_api_key.clone(),
        config.email_from.clone(),
    ));

    let domain_email_sender = Arc::new(DomainEmailSender::new());

    let user_repo_arc = postgres_arc.clone() as Arc<dyn UserRepo>;
    let domain_repo_arc = postgres_arc.clone() as Arc<dyn DomainRepo>;
    let auth_config_repo_arc = postgres_arc.clone() as Arc<dyn DomainAuthConfigRepo>;
    let magic_link_config_repo_arc = postgres_arc.clone() as Arc<dyn DomainAuthMagicLinkRepo>;
    let end_user_repo_arc = postgres_arc.clone() as Arc<dyn DomainEndUserRepo>;

    let auth_use_cases = AuthUseCases::new(
        user_repo_arc.clone(),
        magic_links,
        email.clone(),
        config.app_origin.to_string(),
    );

    let dns_verifier = Arc::new(HickoryDnsVerifier::new());
    let domain_use_cases = DomainUseCases::new(
        domain_repo_arc.clone(),
        dns_verifier,
        "ingress.reauth.dev".to_string(),
    );

    // Initialize cipher for domain auth
    let cipher = ProcessCipher::from_env()?;

    // Get global Resend config for fallback
    let global_resend_api_key = Some(config.resend_api_key.expose_secret().to_string());
    let global_from_email = Some(config.email_from.clone());

    let domain_auth_use_cases = DomainAuthUseCases::new(
        domain_repo_arc,
        auth_config_repo_arc,
        magic_link_config_repo_arc,
        end_user_repo_arc,
        domain_magic_links,
        domain_email_sender,
        cipher,
        global_resend_api_key,
        global_from_email,
    );

    Ok(AppState {
        config: Arc::new(config),
        auth_use_cases: Arc::new(auth_use_cases),
        domain_use_cases: Arc::new(domain_use_cases),
        domain_auth_use_cases: Arc::new(domain_auth_use_cases),
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
