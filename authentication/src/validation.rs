use crate::AccountError;

pub fn validate_username(username: &str) -> Result<(), AccountError> {
    // i want username vars checked without changing their case
    if !(3..=20).contains(&username.len())
        || !username
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || value == b'_')
    {
        return Err(AccountError::InvalidUsername);
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), AccountError> {
    // i want pw length checked without trimming or extra rules
    if !(8..=128).contains(&password.chars().count()) {
        return Err(AccountError::InvalidPassword);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_username_boundaries_and_case() {
        for username in ["Sam", "sam_123", &"a".repeat(20)] {
            assert_eq!(validate_username(username), Ok(()));
        }
    }

    #[test]
    fn rejects_username_length_outside_limits() {
        for username in ["", "a", "ab", &"a".repeat(21)] {
            assert_eq!(
                validate_username(username),
                Err(AccountError::InvalidUsername)
            );
        }
    }

    #[test]
    fn rejects_spaces_symbols_and_non_ascii_names() {
        for username in [" Sam", "Sam ", "sam c", "sam-c", "sam@c", "sam\nc", "éabc"] {
            assert_eq!(
                validate_username(username),
                Err(AccountError::InvalidUsername)
            );
        }
    }

    #[test]
    fn accepts_password_boundaries_without_complexity_rules() {
        for password in ["a".repeat(8), "a".repeat(128), " ".repeat(8), "é".repeat(8)] {
            assert_eq!(validate_password(&password), Ok(()));
        }
    }

    #[test]
    fn rejects_password_length_outside_limits() {
        for password in ["".to_owned(), "a".repeat(7), "a".repeat(129)] {
            assert_eq!(
                validate_password(&password),
                Err(AccountError::InvalidPassword)
            );
        }
    }
}
