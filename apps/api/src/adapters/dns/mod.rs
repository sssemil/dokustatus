use std::net::SocketAddr;

use async_trait::async_trait;
use hickory_resolver::config::{NameServerConfig, ResolverConfig};
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::proto::rr::RecordType;
use hickory_resolver::proto::xfer::Protocol;
use hickory_resolver::TokioResolver;
use tracing::{debug, warn};

use crate::app_error::AppResult;
use crate::use_cases::domain::DnsVerifier;

pub struct HickoryDnsVerifier {
    resolver: TokioResolver,
}

impl HickoryDnsVerifier {
    /// Create resolver using system DNS configuration.
    pub fn new() -> Self {
        let resolver = TokioResolver::builder_tokio().unwrap().build();
        Self { resolver }
    }

    /// Create resolver pointing to a specific DNS server (for local dev with CoreDNS).
    pub fn with_nameserver(addr: SocketAddr) -> Self {
        let mut config = ResolverConfig::new();
        config.add_name_server(NameServerConfig::new(addr, Protocol::Udp));

        let resolver =
            TokioResolver::builder_with_config(config, TokioConnectionProvider::default()).build();
        Self { resolver }
    }
}

#[async_trait]
impl DnsVerifier for HickoryDnsVerifier {
    async fn check_cname(&self, domain: &str, expected_target: &str) -> AppResult<bool> {
        debug!(domain = %domain, expected = %expected_target, "Checking CNAME record");

        // Append trailing dot to make it an FQDN and prevent search domain appending
        let fqdn = if domain.ends_with('.') {
            domain.to_string()
        } else {
            format!("{}.", domain)
        };

        match self.resolver.lookup(&fqdn, RecordType::CNAME).await {
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

        // Append trailing dot to make it an FQDN and prevent search domain appending
        let fqdn = if domain.ends_with('.') {
            domain.to_string()
        } else {
            format!("{}.", domain)
        };

        match self.resolver.lookup(&fqdn, RecordType::TXT).await {
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
