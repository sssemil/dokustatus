use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::application::use_cases::webhook::{WebhookDeliveryWithDetails, WebhookUseCases};
use crate::infra::webhook_signer::sign_webhook_payload;

const POLL_INTERVAL_SECS: u64 = 5;
const STALE_CHECK_INTERVAL_SECS: u64 = 60;
const BATCH_SIZE: i64 = 50;
const MAX_CONCURRENT_DELIVERIES: usize = 10;
const HTTP_TIMEOUT_SECS: u64 = 10;
const RESPONSE_BODY_CAP: usize = 1024;

pub async fn run_webhook_delivery_loop(webhook_uc: Arc<WebhookUseCases>) {
    let mut delivery_ticker = interval(Duration::from_secs(POLL_INTERVAL_SECS));
    let mut stale_ticker = interval(Duration::from_secs(STALE_CHECK_INTERVAL_SECS));

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DELIVERIES));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("failed to build reqwest client");

    info!(
        "Webhook delivery worker started (polling every {}s, max {} concurrent)",
        POLL_INTERVAL_SECS, MAX_CONCURRENT_DELIVERIES
    );

    loop {
        tokio::select! {
            _ = delivery_ticker.tick() => {
                process_batch(&webhook_uc, &client, &semaphore).await;
            }
            _ = stale_ticker.tick() => {
                release_stale(&webhook_uc).await;
            }
        }
    }
}

async fn process_batch(
    webhook_uc: &Arc<WebhookUseCases>,
    client: &reqwest::Client,
    semaphore: &Arc<Semaphore>,
) {
    let deliveries = match webhook_uc.claim_pending_deliveries(BATCH_SIZE).await {
        Ok(d) => d,
        Err(e) => {
            error!(error = %e, "Failed to claim pending webhook deliveries");
            return;
        }
    };

    if deliveries.is_empty() {
        return;
    }

    info!(count = deliveries.len(), "Processing webhook deliveries");

    let mut handles = Vec::with_capacity(deliveries.len());

    for delivery in deliveries {
        let uc = Arc::clone(webhook_uc);
        let client = client.clone();
        let sem = Arc::clone(semaphore);

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.expect("semaphore closed");
            deliver_one(&uc, &client, &delivery).await;
        }));
    }

    for handle in handles {
        if let Err(e) = handle.await {
            error!(error = %e, "Webhook delivery task panicked");
        }
    }
}

async fn deliver_one(
    webhook_uc: &WebhookUseCases,
    client: &reqwest::Client,
    delivery: &WebhookDeliveryWithDetails,
) {
    let secret = match webhook_uc.decrypt_secret(&delivery.secret_encrypted) {
        Ok(s) => s,
        Err(e) => {
            error!(
                delivery_id = %delivery.delivery_id,
                error = %e,
                "Failed to decrypt webhook secret"
            );
            let _ = webhook_uc
                .record_delivery_failure(
                    delivery.delivery_id,
                    delivery.endpoint_id,
                    delivery.attempt_count + 1,
                    None,
                    None,
                    Some("internal: secret decryption failed"),
                    true,
                )
                .await;
            return;
        }
    };

    if let Err(reason) = check_ssrf(&delivery.endpoint_url).await {
        warn!(
            delivery_id = %delivery.delivery_id,
            url = %delivery.endpoint_url,
            reason = %reason,
            "SSRF check failed for webhook URL"
        );
        let _ = webhook_uc
            .record_delivery_failure(
                delivery.delivery_id,
                delivery.endpoint_id,
                delivery.attempt_count + 1,
                None,
                None,
                Some(&format!("SSRF blocked: {}", reason)),
                true,
            )
            .await;
        return;
    }

    let timestamp = chrono::Utc::now().timestamp();
    let signature = sign_webhook_payload(&secret, timestamp, &delivery.payload_raw);

    let result = client
        .post(&delivery.endpoint_url)
        .header("Content-Type", "application/json")
        .header("Reauth-Webhook-Signature", &signature)
        .header("Reauth-Webhook-Id", format!("evt_{}", delivery.event_id))
        .header("Reauth-Webhook-Timestamp", timestamp.to_string())
        .body(delivery.payload_raw.clone())
        .send()
        .await;

    match result {
        Ok(response) => {
            let status = response.status().as_u16() as i32;
            let body = read_body_capped(response, RESPONSE_BODY_CAP).await;

            if (200..300).contains(&status) {
                if let Err(e) = webhook_uc
                    .record_delivery_success(delivery.delivery_id, delivery.endpoint_id, status)
                    .await
                {
                    error!(
                        delivery_id = %delivery.delivery_id,
                        error = %e,
                        "Failed to record webhook success"
                    );
                }
            } else {
                let is_terminal = is_terminal_status(status);
                if let Err(e) = webhook_uc
                    .record_delivery_failure(
                        delivery.delivery_id,
                        delivery.endpoint_id,
                        delivery.attempt_count + 1,
                        Some(status),
                        Some(&body),
                        None,
                        is_terminal,
                    )
                    .await
                {
                    error!(
                        delivery_id = %delivery.delivery_id,
                        error = %e,
                        "Failed to record webhook failure"
                    );
                }
            }
        }
        Err(e) => {
            let error_msg = format!("HTTP error: {}", e);
            let truncated_error = &error_msg[..error_msg.len().min(RESPONSE_BODY_CAP)];
            if let Err(record_err) = webhook_uc
                .record_delivery_failure(
                    delivery.delivery_id,
                    delivery.endpoint_id,
                    delivery.attempt_count + 1,
                    None,
                    None,
                    Some(truncated_error),
                    false,
                )
                .await
            {
                error!(
                    delivery_id = %delivery.delivery_id,
                    error = %record_err,
                    "Failed to record webhook transport error"
                );
            }
        }
    }
}

fn is_terminal_status(status: i32) -> bool {
    // 4xx (client errors) are terminal, except retryable ones
    if (400..500).contains(&status) {
        // 408 Request Timeout, 409 Conflict, 429 Too Many Requests are retryable
        !matches!(status, 408 | 409 | 429)
    } else {
        false
    }
}

async fn check_ssrf(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("invalid URL: {}", e))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?;

    let port = parsed.port_or_known_default().unwrap_or(443);
    let addr_str = format!("{}:{}", host, port);

    let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host(&addr_str)
        .await
        .map_err(|e| format!("DNS resolution failed: {}", e))?
        .collect();

    if addrs.is_empty() {
        return Err("DNS resolved to no addresses".to_string());
    }

    for addr in &addrs {
        if is_private_ip(&addr.ip()) {
            return Err(format!("resolved to private/reserved IP: {}", addr.ip()));
        }
    }

    Ok(())
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()           // 127.0.0.0/8
            || v4.is_private()         // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
            || v4.is_link_local()      // 169.254.0.0/16
            || v4.is_broadcast()       // 255.255.255.255
            || v4.is_unspecified()     // 0.0.0.0
            || v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64 // 100.64.0.0/10 (CGNAT)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()           // ::1
            || v6.is_unspecified()     // ::
            // fc00::/7 (unique local)
            || (v6.segments()[0] & 0xfe00) == 0xfc00
            // fe80::/10 (link-local)
            || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

async fn read_body_capped(mut response: reqwest::Response, cap: usize) -> String {
    let mut buf = Vec::with_capacity(cap.min(4096));

    while let Ok(Some(chunk)) = response.chunk().await {
        let remaining = cap.saturating_sub(buf.len());
        if remaining == 0 {
            break;
        }
        let take = chunk.len().min(remaining);
        buf.extend_from_slice(&chunk[..take]);
    }

    String::from_utf8_lossy(&buf).into_owned()
}

async fn release_stale(webhook_uc: &WebhookUseCases) {
    match webhook_uc.release_stale_deliveries().await {
        Ok(count) if count > 0 => {
            warn!(count, "Released stale webhook deliveries");
        }
        Ok(_) => {}
        Err(e) => {
            error!(error = %e, "Failed to release stale webhook deliveries");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_status_codes() {
        assert!(is_terminal_status(400));
        assert!(is_terminal_status(401));
        assert!(is_terminal_status(403));
        assert!(is_terminal_status(404));
        assert!(is_terminal_status(410));

        assert!(!is_terminal_status(408));
        assert!(!is_terminal_status(409));
        assert!(!is_terminal_status(429));
        assert!(!is_terminal_status(500));
        assert!(!is_terminal_status(502));
        assert!(!is_terminal_status(503));
        assert!(!is_terminal_status(200));
    }

    #[test]
    fn private_ipv4_detection() {
        assert!(is_private_ip(&"127.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"10.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"172.16.0.1".parse().unwrap()));
        assert!(is_private_ip(&"192.168.1.1".parse().unwrap()));
        assert!(is_private_ip(&"169.254.1.1".parse().unwrap()));
        assert!(is_private_ip(&"0.0.0.0".parse().unwrap()));
        assert!(is_private_ip(&"100.64.0.1".parse().unwrap()));

        assert!(!is_private_ip(&"8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip(&"1.1.1.1".parse().unwrap()));
        assert!(!is_private_ip(&"93.184.216.34".parse().unwrap()));
    }

    #[test]
    fn private_ipv6_detection() {
        assert!(is_private_ip(&"::1".parse().unwrap()));
        assert!(is_private_ip(&"::".parse().unwrap()));
        assert!(is_private_ip(&"fc00::1".parse().unwrap()));
        assert!(is_private_ip(&"fd00::1".parse().unwrap()));
        assert!(is_private_ip(&"fe80::1".parse().unwrap()));

        assert!(!is_private_ip(&"2001:db8::1".parse().unwrap()));
        assert!(!is_private_ip(&"2607:f8b0:4004:800::200e".parse().unwrap()));
    }
}
