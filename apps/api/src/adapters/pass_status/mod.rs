use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::OnceCell;

use crate::{
    app_error::AppError,
    use_cases::pass_status::{DocTypeInfo, PassStatusClient, PassStatusInfo, PassStatusMetadata},
};

#[derive(Clone)]
pub struct MunichPassStatusClient {
    client: Client,
    search_url: String,
    info_url: String,
    counter_url: Option<String>,
    prefixes: OnceCell<PrefixConfig>,
}

impl MunichPassStatusClient {
    pub fn new(search_url: String, info_url: String, counter_url: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build reqwest client");
        Self {
            client,
            search_url,
            info_url,
            counter_url,
            prefixes: OnceCell::new(),
        }
    }

    async fn ensure_prefixes(&self) -> Result<&PrefixConfig, AppError> {
        self.prefixes
            .get_or_try_init(|| async {
                let resp = self
                    .client
                    .get(&self.info_url)
                    .header("User-Agent", "dokustatus/1.0")
                    .send()
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                let parsed: InfoResponse = resp
                    .json()
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                let mut map = HashMap::new();
                if let Some(prefixes) = parsed.doc_type_prefix.personalausweis {
                    map.insert(
                        "BUNDESPERSONALAUSWEIS".to_string(),
                        split_prefixes(prefixes),
                    );
                }
                if let Some(prefixes) = parsed.doc_type_prefix.reisepass {
                    map.insert("REISEPASS".to_string(), split_prefixes(prefixes));
                }
                if let Some(prefixes) = parsed.doc_type_prefix.eid {
                    map.insert("EIDKARTE".to_string(), split_prefixes(prefixes));
                }

                Ok(PrefixConfig { prefixes: map })
            })
            .await
    }
}

#[derive(Deserialize)]
struct PassStatusResponse {
    status: Option<String>,
    #[serde(rename = "type")]
    type_label: Option<String>,
    abholort: Option<PickupLocation>,
}

#[derive(Deserialize)]
struct PickupLocation {
    name: Option<String>,
    strasse: Option<String>,
}

#[derive(Deserialize)]
struct InfoResponse {
    #[serde(rename = "docTypePraefix")]
    doc_type_prefix: DocTypePrefix,
}

#[derive(Deserialize)]
struct DocTypePrefix {
    personalausweis: Option<String>,
    reisepass: Option<String>,
    eid: Option<String>,
}

#[derive(Clone)]
struct PrefixConfig {
    prefixes: HashMap<String, Vec<String>>,
}

fn split_prefixes(raw: String) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[async_trait]
impl PassStatusClient for MunichPassStatusClient {
    async fn detect_type(&self, number: &str) -> Result<String, AppError> {
        let prefixes = self.ensure_prefixes().await?;
        let prefix = &number[..4];
        let detected = prefixes
            .prefixes
            .iter()
            .find_map(|(typ, list)| list.iter().find(|p| *p == prefix).map(|_| typ.clone()));

        detected.ok_or_else(|| {
            AppError::InvalidInput(format!(
                "Unsupported document prefix. Allowed prefixes: {:?}",
                prefixes.prefixes
            ))
        })
    }

    async fn metadata(&self) -> Result<PassStatusMetadata, AppError> {
        let prefixes = self.ensure_prefixes().await?;
        let doc_types = prefixes
            .prefixes
            .iter()
            .map(|(code, prefixes)| DocTypeInfo {
                code: code.clone(),
                prefixes: prefixes.clone(),
            })
            .collect();
        Ok(PassStatusMetadata { doc_types })
    }

    async fn fetch_status(&self, number: &str, typ: &str) -> Result<PassStatusInfo, AppError> {
        let body = serde_json::json!({
            "nummer": number,
            "typ": typ
        });

        let resp = self
            .client
            .post(&self.search_url)
            .header("User-Agent", "dokustatus/1.0")
            .header("Content-Type", "application/json")
            .header("Origin", "https://stadt.muenchen.de")
            .header("Referer", "https://stadt.muenchen.de/")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(PassStatusInfo {
                status: Some("UNBEKANNT".to_string()),
                type_label: Some(typ.to_string()),
                pickup: None,
            });
        }

        if let Some(counter_url) = &self.counter_url {
            let client = self.client.clone();
            let counter_url = counter_url.clone();
            tokio::spawn(async move {
                let _ = client
                    .post(counter_url)
                    .header("User-Agent", "dokustatus/1.0")
                    .send()
                    .await;
            });
        }

        let parsed: PassStatusResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let pickup = match (
            parsed.abholort.as_ref().and_then(|p| p.name.clone()),
            parsed.abholort.as_ref().and_then(|p| p.strasse.clone()),
        ) {
            (Some(name), Some(strasse)) if !name.is_empty() || !strasse.is_empty() => {
                Some(format!("{}, {}", name, strasse))
            }
            (Some(name), None) if !name.is_empty() => Some(name),
            (None, Some(strasse)) if !strasse.is_empty() => Some(strasse),
            _ => None,
        };

        Ok(PassStatusInfo {
            status: parsed.status,
            type_label: parsed.type_label,
            pickup,
        })
    }
}
