use serde::{Deserialize, Serialize};

/// User details returned by the Developer API.
///
/// Contains full user information that isn't available in JWT claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDetails {
    /// User ID
    pub id: String,

    /// User's email address
    pub email: String,

    /// User's roles
    pub roles: Vec<String>,

    /// When the email was verified (ISO 8601 format)
    pub email_verified_at: Option<String>,

    /// When the user last logged in (ISO 8601 format)
    pub last_login_at: Option<String>,

    /// Whether the user account is frozen
    pub is_frozen: bool,

    /// Whether the user is on the waitlist whitelist
    pub is_whitelisted: bool,

    /// When the user account was created (ISO 8601 format)
    pub created_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_details_serde() {
        let user = UserDetails {
            id: "user123".to_string(),
            email: "test@example.com".to_string(),
            roles: vec!["user".to_string(), "admin".to_string()],
            email_verified_at: Some("2024-01-15T10:30:00Z".to_string()),
            last_login_at: Some("2024-01-20T14:00:00Z".to_string()),
            is_frozen: false,
            is_whitelisted: true,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
        };

        let json = serde_json::to_string(&user).unwrap();
        let parsed: UserDetails = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "user123");
        assert_eq!(parsed.email, "test@example.com");
        assert!(!parsed.is_frozen);
        assert!(parsed.is_whitelisted);
    }
}
