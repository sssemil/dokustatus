use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::use_cases::domain::DomainUseCases;

const POLL_INTERVAL_SECS: u64 = 10;
const VERIFICATION_TIMEOUT_MINS: i64 = 60;

pub async fn run_domain_verification_loop(domain_use_cases: Arc<DomainUseCases>) {
    let mut ticker = interval(Duration::from_secs(POLL_INTERVAL_SECS));

    info!(
        "Domain verification service started (polling every {}s)",
        POLL_INTERVAL_SECS
    );

    loop {
        ticker.tick().await;

        match domain_use_cases.get_verifying_domains().await {
            Ok(domains) => {
                for domain in domains {
                    // Check if verification has timed out (1 hour)
                    if let Some(started_at) = domain.verification_started_at {
                        let elapsed_mins = (Utc::now().naive_utc() - started_at).num_minutes();
                        if elapsed_mins > VERIFICATION_TIMEOUT_MINS {
                            warn!(
                                domain = %domain.domain,
                                "Verification timed out after {} mins",
                                elapsed_mins
                            );
                            if let Err(e) = domain_use_cases.mark_failed(domain.id).await {
                                error!(
                                    domain = %domain.domain,
                                    error = ?e,
                                    "Failed to mark domain as failed"
                                );
                            }
                            continue;
                        }
                    }

                    // Check DNS
                    match domain_use_cases.check_domain_dns(domain.id).await {
                        Ok(true) => {
                            info!(domain = %domain.domain, "Domain verified successfully");
                        }
                        Ok(false) => {
                            // DNS not ready yet, will retry on next tick
                        }
                        Err(e) => {
                            warn!(domain = %domain.domain, error = ?e, "DNS check failed");
                        }
                    }
                }
            }
            Err(e) => {
                error!(error = ?e, "Failed to fetch verifying domains");
            }
        }
    }
}
