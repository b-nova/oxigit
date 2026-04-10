use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};

use crate::error::{OxigitError, Result};

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut rand_core::OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| OxigitError::InvalidInput(format!("Failed to hash password: {e}")))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| OxigitError::InvalidInput(format!("Invalid password hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Validate username: alphanumeric, hyphens, underscores, 1-39 chars
pub fn validate_username(username: &str) -> Result<()> {
    if username.is_empty() || username.len() > 39 {
        return Err(OxigitError::InvalidInput(
            "Username must be 1-39 characters".into(),
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(OxigitError::InvalidInput(
            "Username may only contain alphanumeric characters, hyphens, and underscores".into(),
        ));
    }
    if username.starts_with('-') || username.ends_with('-') {
        return Err(OxigitError::InvalidInput(
            "Username cannot start or end with a hyphen".into(),
        ));
    }
    Ok(())
}

/// Validate repository name: same rules as username
pub fn validate_repo_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 100 {
        return Err(OxigitError::InvalidInput(
            "Repository name must be 1-100 characters".into(),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(OxigitError::InvalidInput(
            "Repository name may only contain alphanumeric characters, hyphens, underscores, and dots".into(),
        ));
    }
    if name == "." || name == ".." || name.contains("..") {
        return Err(OxigitError::InvalidInput("Invalid repository name".into()));
    }
    Ok(())
}
