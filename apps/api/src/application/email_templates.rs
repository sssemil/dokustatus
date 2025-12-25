use url::Url;

const BRAND_NAME: &str = "reauth";
const COMPANY_NAME: &str = "TQDM Inc.";
const COMPANY_ADDRESS: &str = "1111B S Governors Ave, STE 23256, Dover, DE 19904, USA";

fn origin_label(app_origin: &str) -> String {
    Url::parse(app_origin)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_string()))
        .unwrap_or_else(|| app_origin.to_string())
}

pub fn primary_button(url: &str, label: &str) -> String {
    format!(
        r#"<a href="{url}" style="display:inline-block;padding:12px 18px;background-color:#111827;color:#ffffff;text-decoration:none;border-radius:8px;font-weight:600;">{label}</a>"#
    )
}

pub fn account_created_email(app_origin: &str, domain: &str) -> (String, String) {
    let subject = format!("Welcome to {}", domain);
    let headline = "Your account has been created";
    let lead = format!(
        "You've successfully signed in to <strong>{}</strong>. Your account is now active.",
        domain
    );
    let body = "<p style=\"margin:12px 0 0;color:#374151;\">You can sign in anytime using your email address. We'll send you a secure magic link each time.</p>";
    let reason = format!("you signed up for {}", domain);

    let html = wrap_email(app_origin, headline, &lead, body, &reason, None);
    (subject, html)
}

pub fn account_whitelisted_email(app_origin: &str, domain: &str, login_url: &str) -> (String, String) {
    let subject = format!("You're approved! Access {} now", domain);
    let headline = "You've been approved!";
    let lead = format!(
        "Great news! Your account for <strong>{}</strong> has been approved. You now have full access.",
        domain
    );
    let button = primary_button(login_url, "Sign in now");
    let body = format!(
        r#"{button}<p style="margin:12px 0 0;color:#374151;">Thank you for your patience. You can now sign in and start using all features.</p>"#
    );
    let reason = format!("you were on the waitlist for {}", domain);

    let html = wrap_email(app_origin, headline, &lead, &body, &reason, None);
    (subject, html)
}

pub fn account_frozen_email(app_origin: &str, domain: &str) -> (String, String) {
    let subject = format!("Your {} account has been suspended", domain);
    let headline = "Account suspended";
    let lead = format!(
        "Your account for <strong>{}</strong> has been suspended by an administrator.",
        domain
    );
    let body = "<p style=\"margin:12px 0 0;color:#374151;\">If you believe this is a mistake, please contact the site administrator.</p>";
    let reason = format!("your {} account status changed", domain);

    let html = wrap_email(app_origin, headline, &lead, body, &reason, None);
    (subject, html)
}

pub fn account_unfrozen_email(app_origin: &str, domain: &str, login_url: &str) -> (String, String) {
    let subject = format!("Your {} account has been restored", domain);
    let headline = "Account restored";
    let lead = format!(
        "Good news! Your account for <strong>{}</strong> has been restored. You can sign in again.",
        domain
    );
    let button = primary_button(login_url, "Sign in now");
    let body = format!(
        r#"{button}<p style="margin:12px 0 0;color:#374151;">Your access has been fully restored.</p>"#
    );
    let reason = format!("your {} account status changed", domain);

    let html = wrap_email(app_origin, headline, &lead, &body, &reason, None);
    (subject, html)
}

pub fn account_invited_email(app_origin: &str, domain: &str, login_url: &str) -> (String, String) {
    let subject = format!("You've been invited to {}", domain);
    let headline = "You're invited!";
    let lead = format!(
        "You've been invited to join <strong>{}</strong>. Click the button below to sign in and get started.",
        domain
    );
    let button = primary_button(login_url, "Accept invitation");
    let body = format!(
        r#"{button}<p style="margin:12px 0 0;color:#374151;">Simply click the button above and we'll send you a secure sign-in link.</p>"#
    );
    let reason = format!("an administrator invited you to {}", domain);

    let html = wrap_email(app_origin, headline, &lead, &body, &reason, None);
    (subject, html)
}

pub fn domain_verification_failed_email(app_origin: &str, domain: &str) -> (String, String) {
    let subject = format!("Domain verification failed: {}", domain);
    let headline = "Domain verification failed";
    let lead = format!(
        "We couldn't verify your domain <strong>{}</strong> after checking for one hour.",
        domain
    );
    let body = format!(
        r#"<p style="margin:12px 0 0;color:#374151;">Please check that your DNS records are correctly configured:</p>
        <ul style="margin:12px 0;color:#374151;padding-left:20px;">
          <li>CNAME record for <code>{domain}</code> pointing to <code>ingress.reauth.dev</code></li>
          <li>TXT record for <code>_reauth.{domain}</code> with your project ID</li>
        </ul>
        <p style="margin:12px 0 0;color:#374151;">DNS propagation can sometimes take longer than expected. You can retry verification from your dashboard.</p>"#,
        domain = domain
    );
    let reason = format!("you added {} to your reauth account", domain);

    let html = wrap_email(app_origin, headline, &lead, &body, &reason, None);
    (subject, html)
}

pub fn wrap_email(
    app_origin: &str,
    headline: &str,
    lead: &str,
    body_html: &str,
    reason: &str,
    footer_note: Option<&str>,
) -> String {
    let origin = origin_label(app_origin);
    let reason_label = "Why you got this email";
    let ignore_line = "If you didn't request this, you can safely ignore it.";
    let sent_by = "Sent by";

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
        {company_name} · {company_address}
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
        reason_label = reason_label,
        ignore_line = ignore_line,
        sent_by = sent_by,
        company_name = COMPANY_NAME,
        company_address = COMPANY_ADDRESS,
        footer_note = footer_note,
    )
}
