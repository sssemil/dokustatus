use validator::ValidateEmail;

/// Validates that the input looks like a valid email address
pub fn is_valid_email(email: &str) -> bool {
    let email = email.trim();
    !email.is_empty() && email.validate_email()
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
}
