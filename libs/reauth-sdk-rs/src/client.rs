//! Reauth client implementation.

use reauth_types::{
    DomainEndUserClaims, UserDetails, derive_jwt_secret, peek_domain_id, verify_jwt,
};

use crate::error::ReauthError;
use crate::extract::{Headers, extract_from_cookie, extract_from_header};

/// Configuration for the Reauth client.
#[derive(Debug, Clone)]
pub struct ReauthConfig {
    /// Your verified domain (e.g., "yourdomain.com")
    pub domain: String,

    /// API key for server-to-server authentication (e.g., "sk_live_...")
    pub api_key: String,

    /// Clock skew tolerance in seconds (default: 60)
    pub clock_skew_seconds: Option<u64>,
}

/// Reauth client for server-side authentication.
///
/// Provides methods for verifying JWTs and fetching user details.
pub struct ReauthClient {
    config: ReauthConfig,
    #[cfg(feature = "client")]
    http_client: reqwest::Client,
}

impl ReauthClient {
    /// Create a new Reauth client.
    ///
    /// # Arguments
    /// * `config` - Client configuration including domain and API key
    ///
    /// # Returns
    /// A configured `ReauthClient` or an error if configuration is invalid.
    pub fn new(config: ReauthConfig) -> Result<Self, ReauthError> {
        if config.api_key.is_empty() {
            return Err(ReauthError::Config(
                "apiKey is required. Get one from the Reauth dashboard.".into(),
            ));
        }

        if config.domain.is_empty() {
            return Err(ReauthError::Config("domain is required".into()));
        }

        Ok(Self {
            config,
            #[cfg(feature = "client")]
            http_client: reqwest::Client::new(),
        })
    }

    /// Verify a JWT token locally using HKDF-derived secret.
    ///
    /// No network call required - fast and reliable.
    ///
    /// # Arguments
    /// * `token` - The JWT token to verify
    ///
    /// # Returns
    /// The verified claims or an error.
    ///
    /// # Example
    /// ```rust,ignore
    /// let claims = client.verify_token("eyJ...")?;
    /// println!("User ID: {}", claims.sub);
    /// println!("Roles: {:?}", claims.roles);
    /// println!("Subscription status: {}", claims.subscription.status);
    /// ```
    pub fn verify_token(&self, token: &str) -> Result<DomainEndUserClaims, ReauthError> {
        // 1. Peek at domain_id without verification
        let domain_id = peek_domain_id(token)?;

        // 2. Decode without verification to check domain BEFORE deriving secret
        // (prevents timing side-channel on arbitrary HKDF derivations)
        let unverified_claims = self.peek_claims(token)?;
        if unverified_claims.domain != self.config.domain {
            return Err(ReauthError::DomainMismatch {
                expected: self.config.domain.clone(),
                actual: unverified_claims.domain,
            });
        }

        // 3. Derive secret and verify
        let secret = derive_jwt_secret(&self.config.api_key, &domain_id);
        let clock_skew = self.config.clock_skew_seconds.unwrap_or(60);
        let claims = verify_jwt(token, &secret, clock_skew)?;

        // 4. Double-check domain after verification (defense in depth)
        if claims.domain != self.config.domain {
            return Err(ReauthError::DomainMismatch {
                expected: self.config.domain.clone(),
                actual: claims.domain,
            });
        }

        Ok(claims)
    }

    /// Extract a token from request headers.
    ///
    /// Tries Authorization: Bearer header first, then falls back to cookies.
    ///
    /// # Arguments
    /// * `headers` - Object implementing the `Headers` trait
    ///
    /// # Returns
    /// The token string or None if not found.
    pub fn extract_token<H: Headers>(&self, headers: &H) -> Option<String> {
        // 1. Try Authorization: Bearer header (preferred)
        if let Some(auth) = headers.get_authorization() {
            if let Some(token) = extract_from_header(auth) {
                return Some(token.to_string());
            }
        }

        // 2. Try cookie (fallback for same-origin requests)
        if let Some(cookie) = headers.get_cookie() {
            if let Some(token) = extract_from_cookie(cookie) {
                return Some(token);
            }
        }

        None
    }

    /// Authenticate a request by extracting and verifying the token.
    ///
    /// Combines `extract_token` and `verify_token` for convenience.
    ///
    /// # Arguments
    /// * `headers` - Object implementing the `Headers` trait
    ///
    /// # Returns
    /// The verified claims or an error.
    pub fn authenticate<H: Headers>(
        &self,
        headers: &H,
    ) -> Result<DomainEndUserClaims, ReauthError> {
        let token = self
            .extract_token(headers)
            .ok_or_else(|| ReauthError::InvalidToken("No token found".into()))?;

        self.verify_token(&token)
    }

    /// Get user details by ID from the Developer API.
    ///
    /// Use this when you need full user info like email, frozen status, etc.
    /// that isn't available in the JWT claims.
    ///
    /// # Arguments
    /// * `user_id` - The user ID to fetch
    ///
    /// # Returns
    /// User details or None if not found.
    #[cfg(feature = "client")]
    pub async fn get_user_by_id(&self, user_id: &str) -> Result<Option<UserDetails>, ReauthError> {
        let url = format!(
            "https://reauth.{}/api/developer/{}/users/{}",
            self.config.domain, self.config.domain, user_id
        );

        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&self.config.api_key)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ReauthError::ApiError {
                code: reauth_types::ErrorCode::InvalidApiKey,
                message: "Invalid API key".into(),
            });
        }

        if !response.status().is_success() {
            return Err(ReauthError::ApiError {
                code: reauth_types::ErrorCode::InternalError,
                message: format!("Failed to get user: {}", response.status()),
            });
        }

        // Parse response (snake_case from API)
        #[derive(serde::Deserialize)]
        struct ApiUserDetails {
            id: String,
            email: String,
            roles: Vec<String>,
            email_verified_at: Option<String>,
            last_login_at: Option<String>,
            is_frozen: bool,
            is_whitelisted: bool,
            created_at: Option<String>,
        }

        let data: ApiUserDetails = response.json().await?;

        Ok(Some(UserDetails {
            id: data.id,
            email: data.email,
            roles: data.roles,
            email_verified_at: data.email_verified_at,
            last_login_at: data.last_login_at,
            is_frozen: data.is_frozen,
            is_whitelisted: data.is_whitelisted,
            created_at: data.created_at,
        }))
    }

    /// Peek at claims without verifying signature.
    /// Used internally for domain validation before secret derivation.
    fn peek_claims(&self, token: &str) -> Result<DomainEndUserClaims, ReauthError> {
        use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, errors::Error as JwtError};

        let mut validation = Validation::new(Algorithm::HS256);
        validation.insecure_disable_signature_validation();
        validation.validate_exp = false;

        let token_data = decode::<DomainEndUserClaims>(
            token,
            &DecodingKey::from_secret(b"ignored"),
            &validation,
        )
        .map_err(|e: JwtError| ReauthError::InvalidToken(e.to_string()))?;

        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation_empty_api_key() {
        let result = ReauthClient::new(ReauthConfig {
            domain: "example.com".into(),
            api_key: "".into(),
            clock_skew_seconds: None,
        });

        assert!(matches!(result, Err(ReauthError::Config(_))));
    }

    #[test]
    fn test_config_validation_empty_domain() {
        let result = ReauthClient::new(ReauthConfig {
            domain: "".into(),
            api_key: "sk_live_test".into(),
            clock_skew_seconds: None,
        });

        assert!(matches!(result, Err(ReauthError::Config(_))));
    }

    #[test]
    fn test_valid_config() {
        let result = ReauthClient::new(ReauthConfig {
            domain: "example.com".into(),
            api_key: "sk_live_test".into(),
            clock_skew_seconds: Some(120),
        });

        assert!(result.is_ok());
    }

    // Mock headers for testing
    struct MockHeaders {
        authorization: Option<String>,
        cookie: Option<String>,
    }

    impl Headers for MockHeaders {
        fn get_authorization(&self) -> Option<&str> {
            self.authorization.as_deref()
        }

        fn get_cookie(&self) -> Option<&str> {
            self.cookie.as_deref()
        }
    }

    #[test]
    fn test_extract_token_from_bearer() {
        let client = ReauthClient::new(ReauthConfig {
            domain: "example.com".into(),
            api_key: "sk_live_test".into(),
            clock_skew_seconds: None,
        })
        .unwrap();

        let headers = MockHeaders {
            authorization: Some("Bearer eyJtoken".into()),
            cookie: None,
        };

        assert_eq!(client.extract_token(&headers), Some("eyJtoken".into()));
    }

    #[test]
    fn test_extract_token_from_cookie() {
        let client = ReauthClient::new(ReauthConfig {
            domain: "example.com".into(),
            api_key: "sk_live_test".into(),
            clock_skew_seconds: None,
        })
        .unwrap();

        let headers = MockHeaders {
            authorization: None,
            cookie: Some("other=abc; end_user_access_token=eyJtoken".into()),
        };

        assert_eq!(client.extract_token(&headers), Some("eyJtoken".into()));
    }

    #[test]
    fn test_extract_token_prefers_bearer() {
        let client = ReauthClient::new(ReauthConfig {
            domain: "example.com".into(),
            api_key: "sk_live_test".into(),
            clock_skew_seconds: None,
        })
        .unwrap();

        let headers = MockHeaders {
            authorization: Some("Bearer bearer_token".into()),
            cookie: Some("end_user_access_token=cookie_token".into()),
        };

        // Bearer header takes precedence
        assert_eq!(client.extract_token(&headers), Some("bearer_token".into()));
    }
}
