/// Multi-part TLDs that require special handling
const MULTI_PART_TLDS: &[&str] = &[
    "co.uk", "com.au", "co.nz", "com.br", "co.jp", "org.uk", "net.au", "co.za",
];

/// Check if a domain is a root domain (not a subdomain)
/// Handles multi-part TLDs like .co.uk, .com.au, etc.
pub fn is_root_domain(domain: &str) -> bool {
    let parts: Vec<&str> = domain.split('.').collect();

    // Must have at least 2 parts (name + TLD)
    if parts.len() < 2 {
        return false;
    }

    let domain_lower = domain.to_lowercase();

    for tld in MULTI_PART_TLDS {
        if domain_lower.ends_with(tld) {
            // For multi-part TLDs, root domain has exactly 3 parts
            return parts.len() == 3;
        }
    }

    // For standard TLDs, root domain has exactly 2 parts
    parts.len() == 2
}

/// Extract the root domain from any hostname
/// e.g., "login.example.com" -> "example.com"
/// e.g., "app.staging.example.co.uk" -> "example.co.uk"
pub fn get_root_domain(hostname: &str) -> String {
    let parts: Vec<&str> = hostname.split('.').collect();
    let hostname_lower = hostname.to_lowercase();

    // Handle multi-part TLDs
    for tld in MULTI_PART_TLDS {
        if hostname_lower.ends_with(tld) {
            // For multi-part TLDs, we need 3 parts minimum (domain + tld)
            if parts.len() >= 3 {
                let tld_parts: Vec<&str> = tld.split('.').collect();
                let domain_start = parts.len() - tld_parts.len() - 1;
                return parts[domain_start..].join(".");
            }
        }
    }

    // Standard TLDs: take last 2 parts
    if parts.len() >= 2 {
        return parts[parts.len() - 2..].join(".");
    }

    // Fallback: return as-is
    hostname.to_string()
}

/// Extract root domain from a reauth.* hostname
/// e.g., "reauth.example.com" -> "example.com"
/// Special case: "reauth.dev" stays as "reauth.dev" (it's the actual domain)
pub fn extract_root_from_reauth_hostname(hostname: &str) -> String {
    if hostname.starts_with("reauth.") {
        let remainder = hostname.strip_prefix("reauth.").unwrap_or(hostname);
        // Only strip if remainder is a valid domain (contains at least one dot)
        // This prevents "reauth.dev" from becoming "dev"
        if remainder.contains('.') {
            remainder.to_string()
        } else {
            hostname.to_string()
        }
    } else {
        hostname.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_root_domain() {
        // Standard TLDs
        assert!(is_root_domain("example.com"));
        assert!(is_root_domain("reauth.dev"));
        assert!(!is_root_domain("sub.example.com"));
        assert!(!is_root_domain("deep.sub.example.com"));

        // Multi-part TLDs
        assert!(is_root_domain("example.co.uk"));
        assert!(!is_root_domain("sub.example.co.uk"));
        assert!(is_root_domain("example.com.au"));
    }

    #[test]
    fn test_get_root_domain() {
        // Standard TLDs
        assert_eq!(get_root_domain("login.example.com"), "example.com");
        assert_eq!(get_root_domain("app.staging.example.com"), "example.com");
        assert_eq!(get_root_domain("example.com"), "example.com");

        // Multi-part TLDs
        assert_eq!(get_root_domain("login.example.co.uk"), "example.co.uk");
        assert_eq!(get_root_domain("app.example.co.uk"), "example.co.uk");
        assert_eq!(get_root_domain("example.co.uk"), "example.co.uk");
    }

    #[test]
    fn test_extract_root_from_reauth_hostname() {
        // Normal subdomain extraction
        assert_eq!(extract_root_from_reauth_hostname("reauth.example.com"), "example.com");
        assert_eq!(extract_root_from_reauth_hostname("reauth.foo.co.uk"), "foo.co.uk");

        // Special case: reauth.dev is the actual domain
        assert_eq!(extract_root_from_reauth_hostname("reauth.dev"), "reauth.dev");

        // Non-reauth hostnames pass through
        assert_eq!(extract_root_from_reauth_hostname("example.com"), "example.com");
    }
}
