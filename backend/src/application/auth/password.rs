use argon2::{
    Argon2,
    password_hash::{
        Error as PasswordHashError, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    },
};
use rand_core::OsRng;
use thiserror::Error;

pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);

    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, PasswordError> {
    let parsed_hash = PasswordHash::new(password_hash)?;

    match Argon2::default().verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(PasswordHashError::Password) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("password hashing failed")]
    Hash,
}

impl From<PasswordHashError> for PasswordError {
    fn from(_error: PasswordHashError) -> Self {
        Self::Hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_does_not_store_plaintext_password() {
        let hash = hash_password("correct horse battery staple").expect("hash should succeed");

        assert_ne!(hash, "correct horse battery staple");
        assert!(!hash.contains("correct horse battery staple"));
    }

    #[test]
    fn verifies_matching_password() {
        let hash = hash_password("correct horse battery staple").expect("hash should succeed");

        let verified = verify_password("correct horse battery staple", &hash)
            .expect("verification should succeed");

        assert!(verified);
    }

    #[test]
    fn rejects_non_matching_password() {
        let hash = hash_password("correct horse battery staple").expect("hash should succeed");

        let verified = verify_password("wrong password", &hash).expect("verification should run");

        assert!(!verified);
    }
}
