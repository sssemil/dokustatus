use validator::ValidateEmail;

/// Validates that the input looks like a valid email address
pub fn is_valid_email(email: &str) -> bool {
    let email = email.trim();
    !email.is_empty() && email.validate_email()
}

/// Validates a plan code for URL-friendly characters.
/// Rules:
/// - 1-50 characters
/// - Only lowercase ASCII letters, numbers, hyphens, underscores
/// - Must start with a letter or number (not hyphen/underscore)
/// - No whitespace allowed
pub fn is_valid_plan_code(code: &str) -> bool {
    if code.is_empty() || code.len() > 50 {
        return false;
    }

    // Reject any whitespace
    if code.chars().any(|c| c.is_whitespace()) {
        return false;
    }

    // First character must be alphanumeric
    let first = code.chars().next().unwrap();
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }

    // All characters must be lowercase alphanumeric, hyphen, or underscore
    code.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_emails() {
        assert!(is_valid_email("test@example.com"));
        assert!(is_valid_email("user.name@domain.co.uk"));
        assert!(is_valid_email("user+tag@example.org"));
    }

    #[test]
    fn test_invalid_emails() {
        assert!(!is_valid_email(""));
        assert!(!is_valid_email("   "));
        assert!(!is_valid_email("notanemail"));
        assert!(!is_valid_email("@nodomain.com"));
        assert!(!is_valid_email("spaces in@email.com"));
    }

    #[test]
    fn test_valid_plan_codes() {
        assert!(is_valid_plan_code("basic"));
        assert!(is_valid_plan_code("pro-plan"));
        assert!(is_valid_plan_code("tier_1"));
        assert!(is_valid_plan_code("plan-with-dashes"));
        assert!(is_valid_plan_code("plan_with_underscores"));
        assert!(is_valid_plan_code("plan123"));
        assert!(is_valid_plan_code("123plan"));
        assert!(is_valid_plan_code("a")); // minimum length
        assert!(is_valid_plan_code(&"a".repeat(50))); // maximum length
    }

    #[test]
    fn test_invalid_plan_codes_empty_and_too_long() {
        assert!(!is_valid_plan_code(""));
        assert!(!is_valid_plan_code(&"a".repeat(51)));
    }

    #[test]
    fn test_invalid_plan_codes_whitespace() {
        assert!(!is_valid_plan_code(" basic"));
        assert!(!is_valid_plan_code("basic "));
        assert!(!is_valid_plan_code("basic plan"));
        assert!(!is_valid_plan_code("\tbasic"));
        assert!(!is_valid_plan_code("basic\n"));
    }

    #[test]
    fn test_invalid_plan_codes_leading_separator() {
        assert!(!is_valid_plan_code("-basic"));
        assert!(!is_valid_plan_code("_basic"));
    }

    #[test]
    fn test_invalid_plan_codes_special_characters() {
        assert!(!is_valid_plan_code("plan@code"));
        assert!(!is_valid_plan_code("plan.code"));
        assert!(!is_valid_plan_code("plan/code"));
        assert!(!is_valid_plan_code("plan!"));
        assert!(!is_valid_plan_code("plan$"));
        assert!(!is_valid_plan_code("plan#tag"));
        assert!(!is_valid_plan_code("plan%off"));
    }

    #[test]
    fn test_invalid_plan_codes_uppercase() {
        // Validator expects lowercase (caller should normalize first)
        assert!(!is_valid_plan_code("BASIC"));
        assert!(!is_valid_plan_code("Basic"));
        assert!(!is_valid_plan_code("bAsIc"));
    }

    #[test]
    fn test_invalid_plan_codes_unicode() {
        assert!(!is_valid_plan_code("plän"));
        assert!(!is_valid_plan_code("план")); // Cyrillic
        assert!(!is_valid_plan_code("计划")); // Chinese
        assert!(!is_valid_plan_code("プラン")); // Japanese
    }
}
