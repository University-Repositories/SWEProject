pub mod password;
pub mod session;
pub mod validation;

use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountError {
    InvalidUsername,
    InvalidPassword,
    UsernameTaken,
    InvalidCredentials,
    SessionInvalid,
    Internal,
}

impl fmt::Display for AccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidUsername => "Invalid username",
            Self::InvalidPassword => "Invalid password",
            Self::UsernameTaken => "Username is taken",
            Self::InvalidCredentials => "Invalid username or password",
            Self::SessionInvalid => "Session is invalid",
            Self::Internal => "Internal account error",
        };
        f.write_str(message)
    }
}

impl Error for AccountError {}
