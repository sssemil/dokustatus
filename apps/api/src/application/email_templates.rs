use url::Url;

use crate::application::language::UserLanguage;

const BRAND_NAME: &str = "Dokustatus";
const COMPANY_NAME: &str = "TQDM Inc.";
const COMPANY_ADDRESS: &str = "1111B S Governors Ave, STE 23256, Dover, DE 19904, USA";

fn origin_label(app_origin: &str) -> String {
    Url::parse(app_origin)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_string()))
        .unwrap_or_else(|| app_origin.to_string())
}

fn impressum_url(app_origin: &str, lang: UserLanguage) -> String {
    let base = app_origin.trim_end_matches('/');
    format!("{}/{}/impressum", base, lang.as_str())
}

pub fn primary_button(url: &str, label: &str) -> String {
    format!(
        r#"<a href="{url}" style="display:inline-block;padding:12px 18px;background-color:#111827;color:#ffffff;text-decoration:none;border-radius:8px;font-weight:600;">{label}</a>"#
    )
}

pub fn wrap_email(
    lang: UserLanguage,
    app_origin: &str,
    headline: &str,
    lead: &str,
    body_html: &str,
    reason: &str,
    footer_note: Option<&str>,
) -> String {
    let origin = origin_label(app_origin);
    let copy = match lang {
        UserLanguage::En => (
            "Why you got this email",
            "If you didn't request this, you can safely ignore it.",
            "Sent by",
            "Imprint",
        ),
        UserLanguage::De => (
            "Grund für diese E-Mail",
            "Falls du das nicht warst, kannst du diese Nachricht ignorieren.",
            "Gesendet von",
            "Impressum",
        ),
    };
    let footer_note = footer_note
        .map(|note| {
            format!(
                r#"<p style="margin:8px 0 0;color:#4b5563;font-size:13px;">{}</p>"#,
                note
            )
        })
        .unwrap_or_default();

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
  <body style="background:#f8fafc;margin:0;padding:24px;font-family:Arial,Helvetica,sans-serif;">
    <div style="max-width:560px;margin:0 auto;background:#ffffff;border:1px solid #e5e7eb;border-radius:12px;padding:24px;box-shadow:0 8px 30px rgba(0,0,0,0.04);">
      <div style="font-size:12px;letter-spacing:0.08em;text-transform:uppercase;color:#6b7280;">{brand} - {origin}</div>
      <h1 style="margin:12px 0 8px;font-size:22px;color:#111827;">{headline}</h1>
      <p style="margin:0 0 12px;font-size:15px;color:#111827;line-height:1.6;">{lead}</p>
      {body_html}
      <div style="margin-top:20px;padding-top:16px;border-top:1px solid #e5e7eb;">
        <p style="margin:0 0 6px;font-size:13px;color:#4b5563;">{reason_label}: {reason}.</p>
        <p style="margin:0;font-size:13px;color:#4b5563;">{ignore_line}</p>
        {footer_note}
      </div>
      <p style="margin:14px 0 4px;font-size:12px;color:#9ca3af;">{sent_by} {brand} - {origin}</p>
      <p style="margin:0;font-size:11px;color:#9ca3af;line-height:1.5;">
        {company_name} · {company_address} · <a href="{impressum}" style="color:#6b7280;text-decoration:none;">{impressum_label}</a>
      </p>
    </div>
  </body>
</html>
"#,
        brand = BRAND_NAME,
        origin = origin,
        headline = headline,
        lead = lead,
        body_html = body_html,
        reason = reason,
        reason_label = copy.0,
        ignore_line = copy.1,
        sent_by = copy.2,
        impressum_label = copy.3,
        company_name = COMPANY_NAME,
        company_address = COMPANY_ADDRESS,
        impressum = impressum_url(app_origin, lang),
        footer_note = footer_note,
    )
}
