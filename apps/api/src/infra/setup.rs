use crate::{
    adapters::{
        dns::HickoryDnsVerifier, http::app_state::AppState,
        persistence::domain_role::DomainRoleRepoTrait,
        persistence::enabled_payment_providers::EnabledPaymentProvidersRepoTrait,
    },
    application::use_cases::api_key::{ApiKeyRepoTrait, ApiKeyUseCases},
    application::use_cases::domain_auth::{
        DomainAuthConfigRepoTrait, DomainAuthGoogleOAuthRepoTrait, DomainAuthMagicLinkRepoTrait,
        DomainAuthUseCases, DomainEndUserRepoTrait,
    },
    application::use_cases::domain_billing::{
        BillingPaymentRepoTrait, BillingStripeConfigRepoTrait, DomainBillingUseCases,
        SubscriptionEventRepoTrait, SubscriptionPlanRepoTrait, UserSubscriptionRepoTrait,
    },
    application::use_cases::domain_roles::DomainRolesUseCases,
    application::use_cases::payment_provider_factory::PaymentProviderFactory,
    infra::{
        InfraError, config::AppConfig, crypto::ProcessCipher, domain_email::DomainEmailSender,
        domain_magic_links::DomainMagicLinkStore, oauth_state::OAuthStateStore,
        postgres_persistence, rate_limit::RateLimiter,
    },
    use_cases::domain::{DomainRepoTrait, DomainUseCases},
};
use std::fs::File;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub async fn init_app_state() -> Result<AppState, InfraError> {
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

    // Redis connection for domain magic links and OAuth state
    let redis_client =
        redis::Client::open(config.redis_url.as_str()).map_err(InfraError::RedisConnection)?;
    let redis_manager = redis::aio::ConnectionManager::new(redis_client)
        .await
        .map_err(InfraError::RedisConnection)?;
    let domain_magic_links = Arc::new(DomainMagicLinkStore::new(redis_manager.clone()));
    let oauth_state_store = Arc::new(OAuthStateStore::new(redis_manager));

    let domain_email_sender = Arc::new(DomainEmailSender::new());

    let domain_repo_arc = postgres_arc.clone() as Arc<dyn DomainRepoTrait>;
    let auth_config_repo_arc = postgres_arc.clone() as Arc<dyn DomainAuthConfigRepoTrait>;
    let magic_link_config_repo_arc = postgres_arc.clone() as Arc<dyn DomainAuthMagicLinkRepoTrait>;
    let google_oauth_config_repo_arc =
        postgres_arc.clone() as Arc<dyn DomainAuthGoogleOAuthRepoTrait>;
    let end_user_repo_arc = postgres_arc.clone() as Arc<dyn DomainEndUserRepoTrait>;
    let api_key_repo_arc = postgres_arc.clone() as Arc<dyn ApiKeyRepoTrait>;
    let role_repo_arc = postgres_arc.clone() as Arc<dyn DomainRoleRepoTrait>;

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

    let domain_auth_use_cases = DomainAuthUseCases::new(
        domain_repo_arc.clone(),
        auth_config_repo_arc,
        magic_link_config_repo_arc,
        google_oauth_config_repo_arc,
        end_user_repo_arc.clone(),
        domain_magic_links,
        oauth_state_store,
        domain_email_sender,
        cipher,
        config.fallback_resend_api_key.clone(),
        config.fallback_email_domain.clone(),
        config.fallback_google_client_id.clone(),
        config.fallback_google_client_secret.clone(),
    );

    let api_key_cipher = ProcessCipher::from_env()?;
    let api_key_use_cases = ApiKeyUseCases::new(
        api_key_repo_arc,
        domain_repo_arc.clone(),
        end_user_repo_arc.clone(),
        api_key_cipher,
    );

    let domain_roles_use_cases =
        DomainRolesUseCases::new(domain_repo_arc.clone(), role_repo_arc, end_user_repo_arc);

    // Billing use cases
    let billing_stripe_config_repo = postgres_arc.clone() as Arc<dyn BillingStripeConfigRepoTrait>;
    let enabled_providers_repo = postgres_arc.clone() as Arc<dyn EnabledPaymentProvidersRepoTrait>;
    let subscription_plan_repo = postgres_arc.clone() as Arc<dyn SubscriptionPlanRepoTrait>;
    let user_subscription_repo = postgres_arc.clone() as Arc<dyn UserSubscriptionRepoTrait>;
    let subscription_event_repo = postgres_arc.clone() as Arc<dyn SubscriptionEventRepoTrait>;
    let billing_payment_repo = postgres_arc.clone() as Arc<dyn BillingPaymentRepoTrait>;

    let billing_cipher = ProcessCipher::from_env()?;

    // Create payment provider factory
    let provider_factory = Arc::new(PaymentProviderFactory::new(
        billing_cipher.clone(),
        billing_stripe_config_repo.clone(),
    ));

    // NOTE: No fallback Stripe credentials - each domain must configure their own Stripe account.
    let billing_use_cases = DomainBillingUseCases::new(
        domain_repo_arc,
        billing_stripe_config_repo,
        enabled_providers_repo,
        subscription_plan_repo,
        user_subscription_repo,
        subscription_event_repo,
        billing_payment_repo,
        billing_cipher,
        provider_factory,
    );

    Ok(AppState {
        config: Arc::new(config),
        domain_use_cases: Arc::new(domain_use_cases),
        domain_auth_use_cases: Arc::new(domain_auth_use_cases),
        api_key_use_cases: Arc::new(api_key_use_cases),
        domain_roles_use_cases: Arc::new(domain_roles_use_cases),
        billing_use_cases: Arc::new(billing_use_cases),
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
