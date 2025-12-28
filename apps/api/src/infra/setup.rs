use crate::{
    adapters::{
        dns::HickoryDnsVerifier,
        http::app_state::AppState,
        persistence::domain_role::DomainRoleRepo,
    },
    application::use_cases::api_key::{ApiKeyRepo, ApiKeyUseCases},
    application::use_cases::domain_auth::{
        DomainAuthConfigRepo, DomainAuthMagicLinkRepo, DomainAuthUseCases,
        DomainEndUserRepo,
    },
    application::use_cases::domain_roles::DomainRolesUseCases,
    infra::{
        config::AppConfig,
        crypto::ProcessCipher,
        domain_email::DomainEmailSender,
        domain_magic_links::DomainMagicLinkStore,
        postgres_persistence,
        rate_limit::RateLimiter,
    },
    use_cases::domain::{DomainRepo, DomainUseCases},
};
use sqlx::{PgPool, Row};
use std::fs::File;
use std::sync::Arc;
use tracing::{info, warn};
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

    // Redis connection for domain magic links
    let redis_client = redis::Client::open(config.redis_url.as_str())?;
    let redis_manager = redis::aio::ConnectionManager::new(redis_client).await?;
    let domain_magic_links = Arc::new(DomainMagicLinkStore::new(redis_manager));

    let domain_email_sender = Arc::new(DomainEmailSender::new());

    let domain_repo_arc = postgres_arc.clone() as Arc<dyn DomainRepo>;
    let auth_config_repo_arc = postgres_arc.clone() as Arc<dyn DomainAuthConfigRepo>;
    let magic_link_config_repo_arc = postgres_arc.clone() as Arc<dyn DomainAuthMagicLinkRepo>;
    let end_user_repo_arc = postgres_arc.clone() as Arc<dyn DomainEndUserRepo>;
    let api_key_repo_arc = postgres_arc.clone() as Arc<dyn ApiKeyRepo>;
    let role_repo_arc = postgres_arc.clone() as Arc<dyn DomainRoleRepo>;

    let dns_verifier = Arc::new(match config.dns_server {
        Some(addr) => HickoryDnsVerifier::with_nameserver(addr),
        None => HickoryDnsVerifier::new(),
    });
    let domain_use_cases = DomainUseCases::new(
        domain_repo_arc.clone(),
        dns_verifier,
        config.ingress_domain.clone(),
    );

    // Initialize cipher for domain auth
    let cipher = ProcessCipher::from_env()?;

    // Seed email config for the main domain (reauth.dev) if not already set
    seed_main_domain_email_config(postgres_arc.pool(), &cipher, &config.main_domain).await?;

    let domain_auth_use_cases = DomainAuthUseCases::new(
        domain_repo_arc.clone(),
        auth_config_repo_arc,
        magic_link_config_repo_arc,
        end_user_repo_arc.clone(),
        domain_magic_links,
        domain_email_sender,
        cipher,
    );

    let api_key_use_cases = ApiKeyUseCases::new(
        api_key_repo_arc,
        domain_repo_arc.clone(),
        end_user_repo_arc.clone(),
    );

    let domain_roles_use_cases = DomainRolesUseCases::new(
        domain_repo_arc,
        role_repo_arc,
        end_user_repo_arc,
    );

    Ok(AppState {
        config: Arc::new(config),
        domain_use_cases: Arc::new(domain_use_cases),
        domain_auth_use_cases: Arc::new(domain_auth_use_cases),
        api_key_use_cases: Arc::new(api_key_use_cases),
        domain_roles_use_cases: Arc::new(domain_roles_use_cases),
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

/// Seeds the main domain (e.g., reauth.dev) with email config if not already set.
/// This is required because we no longer have a global fallback Resend API key.
async fn seed_main_domain_email_config(
    pool: &PgPool,
    cipher: &ProcessCipher,
    main_domain: &str,
) -> anyhow::Result<()> {
    // Check if the main domain exists
    let domain_row = sqlx::query("SELECT id FROM domains WHERE domain = $1")
        .bind(main_domain)
        .fetch_optional(pool)
        .await?;

    let domain_id: uuid::Uuid = match domain_row {
        Some(row) => row.get("id"),
        None => {
            warn!(
                "Main domain '{}' not found in database - skipping email config seeding",
                main_domain
            );
            return Ok(());
        }
    };

    // Check if email config already exists for this domain
    let existing = sqlx::query("SELECT id FROM domain_auth_magic_link WHERE domain_id = $1")
        .bind(domain_id)
        .fetch_optional(pool)
        .await?;

    if existing.is_some() {
        info!(
            "Email config already exists for '{}' - skipping seeding",
            main_domain
        );
        return Ok(());
    }

    // Get the Resend API key from environment (loaded from secret in production)
    let resend_api_key = match std::env::var("REAUTH_DEV_RESEND_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            warn!(
                "REAUTH_DEV_RESEND_API_KEY not set - cannot seed email config for '{}'",
                main_domain
            );
            return Ok(());
        }
    };

    // Encrypt the API key
    let encrypted_key = cipher.encrypt(&resend_api_key)?;
    let from_email = format!("noreply@{}", main_domain);

    // Insert the email config
    sqlx::query(
        r#"
        INSERT INTO domain_auth_magic_link (domain_id, resend_api_key_encrypted, from_email)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(domain_id)
    .bind(&encrypted_key)
    .bind(&from_email)
    .execute(pool)
    .await?;

    info!(
        "Seeded email config for '{}' with from_email '{}'",
        main_domain, from_email
    );

    Ok(())
}
