//! Token extraction utilities.

/// Trait for accessing HTTP headers in a framework-agnostic way.
///
/// Implement this trait for your framework's header type to use
/// `ReauthClient::authenticate()`.
///
/// # Example
///
/// ```rust,ignore
/// use reauth_sdk::Headers;
///
/// // For axum
/// impl Headers for axum::http::HeaderMap {
///     fn get_authorization(&self) -> Option<&str> {
///         self.get("authorization")
///             .and_then(|v| v.to_str().ok())
///     }
///
///     fn get_cookie(&self) -> Option<&str> {
///         self.get("cookie")
///             .and_then(|v| v.to_str().ok())
///     }
/// }
/// ```
pub trait Headers {
    /// Get the Authorization header value.
    fn get_authorization(&self) -> Option<&str>;

    /// Get the Cookie header value.
    fn get_cookie(&self) -> Option<&str>;
}

/// Extract token from Authorization Bearer header.
pub fn extract_from_header(auth_header: &str) -> Option<&str> {
    auth_header.strip_prefix("Bearer ")
}

/// Extract token from cookie header.
///
/// Looks for the `end_user_access_token` cookie.
pub fn extract_from_cookie(cookie_header: &str) -> Option<String> {
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(value) = cookie.strip_prefix("end_user_access_token=") {
            // URL decode the value
            return Some(
                urlencoding_decode(value)
                    .unwrap_or_else(|_| value.to_string()),
            );
        }
    }
    None
}

/// Simple URL decoding (handles %XX sequences).
fn urlencoding_decode(s: &str) -> Result<String, ()> {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            return Err(());
        }
        result.push(c);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_from_header() {
        assert_eq!(
            extract_from_header("Bearer eyJtoken"),
            Some("eyJtoken")
        );
        assert_eq!(extract_from_header("bearer eyJtoken"), None);
        assert_eq!(extract_from_header("Basic xyz"), None);
        assert_eq!(extract_from_header(""), None);
    }

    #[test]
    fn test_extract_from_cookie() {
        let cookie = "session=abc; end_user_access_token=eyJtoken; other=xyz";
        assert_eq!(
            extract_from_cookie(cookie),
            Some("eyJtoken".to_string())
        );
    }

    #[test]
    fn test_extract_from_cookie_not_found() {
        let cookie = "session=abc; other=xyz";
        assert_eq!(extract_from_cookie(cookie), None);
    }

    #[test]
    fn test_extract_from_cookie_url_encoded() {
        let cookie = "end_user_access_token=eyJ%3Dtoken";
        assert_eq!(
            extract_from_cookie(cookie),
            Some("eyJ=token".to_string())
        );
    }
}
