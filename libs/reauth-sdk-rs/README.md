# reauth-sdk

Rust SDK for [Reauth](https://reauth.dev) authentication.

## Features

- **Local JWT verification** - Verify tokens without network calls using HKDF-derived secrets
- **Token extraction** - Extract tokens from Authorization headers or cookies
- **Developer API** - Fetch full user details via the Developer API (optional `client` feature)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
reauth-sdk = "0.1"
```

Or with only core verification (no HTTP client):

```toml
[dependencies]
reauth-sdk = { version = "0.1", default-features = false }
```

## Quick Start

```rust
use reauth_sdk::{ReauthClient, ReauthConfig, ReauthError};

fn main() -> Result<(), ReauthError> {
    // Create client
    let client = ReauthClient::new(ReauthConfig {
        domain: "yourdomain.com".to_string(),
        api_key: "sk_live_...".to_string(),
        clock_skew_seconds: None, // Default: 60 seconds
    })?;

    // Verify a token
    let claims = client.verify_token("eyJ...")?;
    println!("User ID: {}", claims.sub);
    println!("Roles: {:?}", claims.roles);
    println!("Subscription: {}", claims.subscription.status);

    Ok(())
}
```

## Extracting Tokens from Requests

Implement the `Headers` trait for your framework:

```rust
use reauth_sdk::Headers;

// Example for axum
impl Headers for axum::http::HeaderMap {
    fn get_authorization(&self) -> Option<&str> {
        self.get("authorization")
            .and_then(|v| v.to_str().ok())
    }

    fn get_cookie(&self) -> Option<&str> {
        self.get("cookie")
            .and_then(|v| v.to_str().ok())
    }
}
```

Then use `authenticate`:

```rust
let claims = client.authenticate(&headers)?;
```

## Axum Middleware Example

```rust
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use reauth_sdk::{ReauthClient, DomainEndUserClaims};

pub async fn require_auth<B>(
    State(reauth): State<ReauthClient>,
    mut request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    // Extract token from headers
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Verify token
    let claims = reauth
        .verify_token(token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Add claims to request extensions
    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}
```

## Fetching User Details

For full user information (email, frozen status, etc.):

```rust
// Requires `client` feature (enabled by default)
let user = client.get_user_by_id(&claims.sub).await?;
if let Some(user) = user {
    println!("Email: {}", user.email);
    println!("Frozen: {}", user.is_frozen);
}
```

## Types

The SDK re-exports types from `reauth-types`:

- `DomainEndUserClaims` - JWT claims structure
- `SubscriptionClaims` - Subscription info in JWT
- `SubscriptionStatus` - Subscription status enum
- `UserDetails` - Full user details from API

## Error Handling

```rust
use reauth_sdk::ReauthError;

match client.verify_token(token) {
    Ok(claims) => println!("User: {}", claims.sub),
    Err(ReauthError::DomainMismatch { expected, actual }) => {
        println!("Wrong domain: expected {}, got {}", expected, actual);
    }
    Err(ReauthError::Jwt(e)) => {
        println!("JWT error: {}", e);
    }
    Err(e) => println!("Other error: {}", e),
}
```

## License

MIT
