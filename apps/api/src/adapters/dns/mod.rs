use async_trait::async_trait;
use hickory_resolver::proto::rr::RecordType;
use hickory_resolver::TokioResolver;
use tracing::{debug, warn};

use crate::app_error::AppResult;
use crate::use_cases::domain::DnsVerifier;

pub struct HickoryDnsVerifier {
    resolver: TokioResolver,
}

impl HickoryDnsVerifier {
    pub fn new() -> Self {
        let resolver = TokioResolver::builder_tokio().unwrap().build();
        Self { resolver }
    }
}

#[async_trait]
impl DnsVerifier for HickoryDnsVerifier {
    async fn check_cname(&self, domain: &str, expected_target: &str) -> AppResult<bool> {
        debug!(domain = %domain, expected = %expected_target, "Checking CNAME record");

        match self.resolver.lookup(domain, RecordType::CNAME).await {
            Ok(lookup) => {
                for record in lookup.records() {
                    if let Some(cname) = record.data().as_cname() {
                        let target = cname.to_string();
                        let target_normalized = target.trim_end_matches('.');
                        let expected_normalized = expected_target.trim_end_matches('.');

                        debug!(target = %target_normalized, expected = %expected_normalized, "Found CNAME");

                        if target_normalized.eq_ignore_ascii_case(expected_normalized) {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }
            Err(e) => {
                warn!(domain = %domain, error = %e, "CNAME lookup failed");
                Ok(false)
            }
        }
    }

    async fn check_txt(&self, domain: &str, expected_value: &str) -> AppResult<bool> {
        debug!(domain = %domain, expected = %expected_value, "Checking TXT record");

        match self.resolver.lookup(domain, RecordType::TXT).await {
            Ok(lookup) => {
                for record in lookup.records() {
                    if let Some(txt) = record.data().as_txt() {
                        let txt_data = txt.to_string();
                        debug!(found = %txt_data, expected = %expected_value, "Found TXT");

                        if txt_data.contains(expected_value) {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }
            Err(e) => {
                warn!(domain = %domain, error = %e, "TXT lookup failed");
                Ok(false)
            }
        }
    }
}
