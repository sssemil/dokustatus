use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

use crate::infra::http_client;
use crate::{
    app_error::{AppError, AppResult},
    application::use_cases::domain_auth::DomainEmailSender as DomainEmailSenderTrait,
};

/// Email sender that accepts API key and from_email as parameters,
/// allowing different domains to use their own Resend configuration.
#[derive(Clone)]
pub struct DomainEmailSender {
    client: Client,
}

impl DomainEmailSender {
    pub fn new() -> Self {
        Self {
            client: http_client::build_client(),
        }
    }
}

impl Default for DomainEmailSender {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct ResendReq<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: &'a str,
    html: &'a str,
}

#[async_trait]
impl DomainEmailSenderTrait for DomainEmailSender {
    async fn send(
        &self,
        api_key: &str,
        from_email: &str,
        to: &str,
        subject: &str,
        html: &str,
    ) -> AppResult<()> {
        let body = ResendReq {
            from: from_email,
            to: [to],
            subject,
            html,
        };

        self.client
            .post("https://api.resend.com/emails")
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to send email: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(format!("Email API error: {e}")))?;

        Ok(())
    }
}
