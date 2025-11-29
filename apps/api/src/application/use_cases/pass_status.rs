use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use tracing::{instrument, warn};
use uuid::Uuid;

use crate::{
    app_error::{AppError, AppResult},
    application::{
        dictionaries::{doc_type_label, pickup_label, status_copy, t},
        email_templates::{primary_button, wrap_email},
        language::UserLanguage,
    },
    use_cases::user::{EmailSender, UserProfile, UserRepo},
};

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTrack {
    pub id: Uuid,
    pub user_id: Uuid,
    pub number: String,
    pub last_status: Option<String>,
    pub last_pickup: Option<String>,
    pub last_checked_at: Option<NaiveDateTime>,
    pub status_changed_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    #[sqlx(skip)]
    #[serde(default)]
    pub typ: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PassStatusInfo {
    pub status: Option<String>,
    pub type_label: Option<String>,
    pub pickup: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusCheckResult {
    pub id: Uuid,
    pub number: String,
    pub typ: Option<String>,
    pub status: Option<String>,
    pub pickup: Option<String>,
    pub changed: bool,
    pub checked_at: NaiveDateTime,
    pub stopped: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocTypeInfo {
    pub code: String,
    pub prefixes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PassStatusMetadata {
    pub doc_types: Vec<DocTypeInfo>,
}

#[async_trait]
pub trait PassStatusRepo: Send + Sync {
    async fn upsert_track(&self, user_id: Uuid, number: &str) -> AppResult<DocumentTrack>;
    async fn list_tracks_for_user(&self, user_id: Uuid) -> AppResult<Vec<DocumentTrack>>;
    async fn list_all_tracks(&self) -> AppResult<Vec<DocumentTrack>>;
    async fn delete_track(&self, user_id: Uuid, track_id: Uuid) -> AppResult<()>;
    async fn save_status(
        &self,
        track_id: Uuid,
        status: Option<String>,
        pickup: Option<String>,
        checked_at: NaiveDateTime,
    ) -> AppResult<()>;
}

#[async_trait]
pub trait PassStatusClient: Send + Sync {
    async fn detect_type(&self, number: &str) -> AppResult<String>;
    async fn metadata(&self) -> AppResult<PassStatusMetadata>;
    async fn fetch_status(&self, number: &str, typ: &str) -> AppResult<PassStatusInfo>;
}

#[derive(Clone)]
pub struct PassStatusUseCases {
    repo: Arc<dyn PassStatusRepo>,
    email: Arc<dyn EmailSender>,
    status_client: Arc<dyn PassStatusClient>,
    user_repo: Arc<dyn UserRepo>,
    refresh_interval_secs: i64,
    max_documents_per_user: usize,
    app_origin: String,
}

impl PassStatusUseCases {
    pub fn new(
        repo: Arc<dyn PassStatusRepo>,
        email: Arc<dyn EmailSender>,
        status_client: Arc<dyn PassStatusClient>,
        user_repo: Arc<dyn UserRepo>,
        refresh_interval_secs: i64,
        max_documents_per_user: usize,
        app_origin: String,
    ) -> Self {
        Self {
            repo,
            email,
            status_client,
            user_repo,
            refresh_interval_secs,
            max_documents_per_user,
            app_origin,
        }
    }

    #[instrument(skip(self))]
    pub async fn add_track(&self, user_id: Uuid, number: &str) -> AppResult<DocumentTrack> {
        let user = self
            .user_repo
            .get_profile_by_id(user_id)
            .await?
            .ok_or(AppError::InvalidCredentials)?;
        let lang = UserLanguage::from_raw(Some(&user.language));

        let existing = self.repo.list_tracks_for_user(user_id).await?;
        if existing.len() >= self.max_documents_per_user {
            return Err(AppError::TooManyDocuments);
        }

        let normalized_number = normalize_number(number)?;
        let normalized_typ = self.status_client.detect_type(&normalized_number).await?;
        let status_info = self
            .status_client
            .fetch_status(&normalized_number, &normalized_typ)
            .await?;

        if status_info.status.is_none() {
            return Err(AppError::InvalidInput("Invalid tracking number".into()));
        }

        let mut track = self.repo.upsert_track(user_id, &normalized_number).await?;
        let now = Utc::now().naive_utc();
        self.repo
            .save_status(
                track.id,
                status_info.status.clone(),
                status_info.pickup.clone(),
                now,
            )
            .await?;

        track.typ = Some(normalized_typ.clone());
        let subject = match lang {
            UserLanguage::En => {
                t(lang, "emails.tracking.subjectStart") + &format!(" {}", normalized_number)
            }
            UserLanguage::De => {
                t(lang, "emails.tracking.subjectStart") + &format!(" {}", normalized_number)
            }
        };
        let status_copy = status_copy(lang, status_info.status.as_deref());
        let pickup_line = status_info
            .pickup
            .as_deref()
            .map(|p| {
                let label = pickup_label(lang);
                format!(
                    "<p style=\"margin:0 0 8px;color:#374151;\">{}: {}</p>",
                    label, p
                )
            })
            .unwrap_or_default();
        let status_line = format!(
            "<p style=\"margin:0 0 8px;font-weight:600;color:#111827;\">{}</p>",
            status_copy.title
        );
        let status_msg = status_copy
            .message
            .as_ref()
            .map(|m| format!("<p style=\"margin:0 0 8px;color:#374151;\">{}</p>", m))
            .unwrap_or_default();
        let details = format!(
            "{status_line}{status_msg}<p style=\"margin:0 0 8px;color:#374151;\">{}: {}</p>{pickup_line}",
            doc_type_label(lang),
            status_info
                .type_label
                .as_deref()
                .unwrap_or(normalized_typ.as_str())
        );
        let cta_label = t(lang, "emails.tracking.ctaOpen");
        let cta = primary_button(self.app_origin.trim_end_matches('/'), &cta_label);
        let headline = t(lang, "emails.tracking.headlineStart");
        let lead = t(lang, "emails.tracking.leadStart").replace("{number}", &normalized_number);
        let reason = t(lang, "emails.tracking.reasonStart")
            .replace("{app}", self.app_origin.trim_end_matches('/'));
        let footer = t(lang, "emails.tracking.footerStart");
        let body = wrap_email(
            lang,
            &self.app_origin,
            &headline,
            &lead,
            &format!(
                "<div style=\"margin:12px 0 0;\">{details}<div style=\"margin-top:16px;\">{cta}</div></div>"
            ),
            &reason,
            Some(&footer),
        );
        self.email.send(&user.email, &subject, &body).await?;

        Ok(DocumentTrack {
            last_status: status_info.status,
            last_pickup: status_info.pickup,
            last_checked_at: Some(now),
            ..track
        })
    }

    pub async fn list_tracks(&self, user_id: Uuid) -> AppResult<Vec<DocumentTrack>> {
        let mut tracks = self.repo.list_tracks_for_user(user_id).await?;
        for track in tracks.iter_mut() {
            if track.typ.is_none() {
                if let Ok(typ) = self.status_client.detect_type(&track.number).await {
                    track.typ = Some(typ);
                }
            }
        }
        Ok(tracks)
    }

    pub async fn delete_track(&self, user_id: Uuid, track_id: Uuid) -> AppResult<()> {
        self.repo.delete_track(user_id, track_id).await
    }

    #[instrument(skip(self))]
    pub async fn check_and_notify(&self, user_id: Uuid) -> AppResult<Vec<StatusCheckResult>> {
        let mut tracks = self.repo.list_tracks_for_user(user_id).await?;
        let now = Utc::now().naive_utc();
        let mut results = Vec::new();

        for track in tracks.iter_mut() {
            if track.typ.is_none() {
                track.typ = self.status_client.detect_type(&track.number).await.ok();
            }
            let status = track.last_status.clone();
            let stopped = is_final(status.as_deref());
            results.push(StatusCheckResult {
                id: track.id,
                number: track.number.clone(),
                typ: track.typ.clone(),
                status,
                pickup: track.last_pickup.clone(),
                changed: false,
                checked_at: track.last_checked_at.unwrap_or(now),
                stopped,
            });
        }

        Ok(results)
    }

    pub async fn metadata(&self) -> AppResult<PassStatusMetadata> {
        self.status_client.metadata().await
    }

    #[instrument(skip(self))]
    pub async fn check_all_and_notify(&self) -> AppResult<Vec<StatusCheckResult>> {
        let app_home = self.app_origin.trim_end_matches('/');
        let tracks = self.repo.list_all_tracks().await?;
        let now = Utc::now().naive_utc();
        let mut results = Vec::new();
        let mut user_cache: HashMap<Uuid, UserProfile> = HashMap::new();

        for track in tracks {
            if let Some(last_checked) = track.last_checked_at
                && (now - last_checked).num_seconds() < self.refresh_interval_secs
            {
                // Not due yet; return cached state.
                let status = track.last_status.clone();
                let stopped = is_final(status.as_deref());
                results.push(StatusCheckResult {
                    id: track.id,
                    number: track.number,
                    typ: None,
                    status,
                    pickup: track.last_pickup.clone(),
                    changed: false,
                    checked_at: last_checked,
                    stopped,
                });
                continue;
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            // Simple staggering: wait a short moment between API calls to avoid burst.

            let normalized_typ = match self.status_client.detect_type(&track.number).await {
                Ok(t) => t,
                Err(err) => {
                    warn!(error = ?err, track_id = %track.id, "detect_type failed; returning cached status");
                    let status = track.last_status.clone();
                    let stopped = is_final(status.as_deref());
                    results.push(StatusCheckResult {
                        id: track.id,
                        number: track.number,
                        typ: None,
                        status,
                        pickup: track.last_pickup.clone(),
                        changed: false,
                        checked_at: track.last_checked_at.unwrap_or(now),
                        stopped,
                    });
                    continue;
                }
            };

            let status_info = match self
                .status_client
                .fetch_status(&track.number, &normalized_typ)
                .await
            {
                Ok(info) => info,
                Err(err) => {
                    warn!(error = ?err, track_id = %track.id, "fetch_status failed; returning cached status");
                    let status = track.last_status.clone();
                    let stopped = is_final(status.as_deref());
                    results.push(StatusCheckResult {
                        id: track.id,
                        number: track.number,
                        typ: None,
                        status,
                        pickup: track.last_pickup.clone(),
                        changed: false,
                        checked_at: track.last_checked_at.unwrap_or(now),
                        stopped,
                    });
                    continue;
                }
            };

            let changed = status_info.status.is_some() && status_info.status != track.last_status;
            let final_state = is_final(status_info.status.as_deref());

            if let Err(err) = self
                .repo
                .save_status(
                    track.id,
                    status_info.status.clone(),
                    status_info.pickup.clone(),
                    now,
                )
                .await
            {
                warn!(error = ?err, track_id = %track.id, "persisting status failed; returning cached status");
                let status = track.last_status.clone();
                let stopped = is_final(status.as_deref());
                results.push(StatusCheckResult {
                    id: track.id,
                    number: track.number,
                    typ: Some(normalized_typ),
                    status,
                    pickup: track.last_pickup.clone(),
                    changed: false,
                    checked_at: track.last_checked_at.unwrap_or(now),
                    stopped,
                });
                continue;
            }

            if changed {
                let user = match user_cache.get(&track.user_id) {
                    Some(u) => u.clone(),
                    None => {
                        let Some(profile) = self.user_repo.get_profile_by_id(track.user_id).await?
                        else {
                            continue;
                        };
                        user_cache.insert(track.user_id, profile.clone());
                        profile
                    }
                };
                let lang = UserLanguage::from_raw(Some(&user.language));
                let status_copy = status_copy(lang, status_info.status.as_deref());
                let subject = t(lang, "emails.tracking.subjectUpdate")
                    + &format!(
                        " {} ({})",
                        track.number,
                        status_info.status.as_deref().unwrap_or("Unbekannt")
                    );
                let pickup_line = status_info
                    .pickup
                    .as_deref()
                    .map(|p| {
                        let label = pickup_label(lang);
                        format!(
                            "<p style=\"margin:0 0 8px;color:#374151;\">{}: {}</p>",
                            label, p
                        )
                    })
                    .unwrap_or_default();
                let status_line = format!(
                    "<p style=\"margin:0 0 8px;font-weight:600;color:#111827;\">{}</p>",
                    status_copy.title
                );
                let status_msg = status_copy
                    .message
                    .as_ref()
                    .map(|m| format!("<p style=\"margin:0 0 8px;color:#374151;\">{}</p>", m))
                    .unwrap_or_default();
                let details = format!(
                    "{status_line}{status_msg}<p style=\"margin:0 0 8px;color:#374151;\">{}: {}</p>{pickup_line}",
                    doc_type_label(lang),
                    status_info
                        .type_label
                        .as_deref()
                        .unwrap_or(normalized_typ.as_str())
                );
                let cta_label = t(lang, "emails.tracking.ctaView");
                let cta = primary_button(app_home, &cta_label);
                let headline = t(lang, "emails.tracking.headlineUpdate");
                let lead = t(lang, "emails.tracking.leadUpdate").replace("{number}", &track.number);
                let reason = t(lang, "emails.tracking.reasonUpdate").replace("{app}", app_home);
                let footer = t(lang, "emails.tracking.footerUpdate");
                let body = wrap_email(
                    lang,
                    &self.app_origin,
                    &headline,
                    &lead,
                    &format!(
                        "<div style=\"margin:12px 0 0;\">{details}<div style=\"margin-top:16px;\">{cta}</div></div>"
                    ),
                    &reason,
                    Some(&footer),
                );
                if let Err(err) = self.email.send(&user.email, &subject, &body).await {
                    warn!(error = ?err, track_id = %track.id, "sending email failed");
                }
            }

            results.push(StatusCheckResult {
                id: track.id,
                number: track.number,
                typ: Some(normalized_typ),
                status: status_info.status,
                pickup: status_info.pickup,
                changed,
                checked_at: now,
                stopped: final_state,
            });
        }

        Ok(results)
    }
}

fn normalize_number(input: &str) -> AppResult<String> {
    let trimmed = input.trim();
    let is_valid = trimmed.len() == 10 && trimmed.chars().all(|c| c.is_ascii_alphanumeric());
    if is_valid {
        Ok(trimmed.to_uppercase())
    } else {
        Err(AppError::InvalidInput(
            "Invalid tracking number format. Expected 10 alphanumeric characters.".into(),
        ))
    }
}

fn is_final(status: Option<&str>) -> bool {
    matches!(
        status,
        Some("AUSGEHAENDIGT") | Some("DIREKTVERSAND_ZUGESTELLT")
    )
}
