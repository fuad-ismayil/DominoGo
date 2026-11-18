//! Session token issuing/validation and the `AuthUser` extractor.
//!
//! Argon2id password storage with transparent legacy SHA-256 migration.

use crate::domain::AppState;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header::AUTHORIZATION, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub username: String,
    pub is_guest: i64,
    pub token: String,
}

pub fn hash_password(password: &str) -> Result<String, String> {
    use argon2::{password_hash::SaltString, PasswordHasher};
    let id = uuid::Uuid::new_v4();
    let salt = SaltString::encode_b64(id.as_bytes()).map_err(|e| e.to_string())?;
    argon2::Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

pub fn verify_password(password: &str, stored: &str) -> bool {
    use argon2::{PasswordHash, PasswordVerifier};
    if stored.starts_with("$argon2") {
        return PasswordHash::new(stored).ok().is_some_and(|hash| {
            argon2::Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok()
        });
    }
    let expected = hex::encode(Sha256::digest(password.as_bytes()));
    stored.len() == 64
        && expected
            .bytes()
            .zip(stored.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}

/// `crypto.randomBytes(32).toString('hex')` equivalent: two UUIDv4 payloads
/// concatenated as hex (64 lowercase hex chars).
pub fn generate_session_token() -> String {
    let a = uuid::Uuid::new_v4();
    let b = uuid::Uuid::new_v4();
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(a.as_bytes());
    bytes[16..].copy_from_slice(b.as_bytes());
    hex::encode(bytes)
}

/// `crypto.randomUUID()` equivalent.
pub fn new_user_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub type AuthRejection = (axum::http::StatusCode, Json<serde_json::Value>);

fn unauthorized(msg: &str) -> AuthRejection {
    (
        axum::http::StatusCode::UNAUTHORIZED,
        Json(json!({ "error": msg })),
    )
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let header: Option<&HeaderValue> = parts.headers.get(AUTHORIZATION);
        let token = header
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(<str>::to_string);
        let Some(token) = token else {
            return Err(unauthorized("Unauthorized"));
        };

        let app = state;

        // Same query as the TS `authenticate` middleware.
        let user_id: Option<String> = app.db.query_one(
            "SELECT user_id FROM sessions WHERE id = ? AND (expires_at IS NULL OR expires_at > datetime('now'))",
            [&token],
            |r| r.get(0),
        ).unwrap_or_default();

        let Some(user_id) = user_id else {
            app.db
                .execute("DELETE FROM sessions WHERE id = ?", [&token])
                .ok();
            return Err(unauthorized("Invalid or expired session"));
        };

        let row: Option<(String, String, i64)> = app
            .db
            .query_one(
                "SELECT id, username, is_guest FROM users WHERE id = ?",
                [&user_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap_or_default();

        let Some((id, username, is_guest)) = row else {
            return Err(unauthorized("User not found"));
        };

        app.db
            .execute(
                "UPDATE users SET last_active_at = CURRENT_TIMESTAMP WHERE id = ?",
                [&id],
            )
            .ok();
        Ok(AuthUser {
            id,
            username,
            is_guest,
            token,
        })
    }
}

/// Uniform internal-error response mirroring TS `res.status(500).json({ error: 'Internal server error' })`.
pub fn internal_error(message: &str) -> Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": message })),
    )
        .into_response()
}

/// rusqlite unique violation check independent of error-code plumbing.
pub fn is_unique_violation(err: &rusqlite::Error) -> bool {
    err.to_string().contains("UNIQUE constraint failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn salted_passwords_and_legacy_migration() {
        let a = hash_password("secure-passphrase").unwrap();
        let b = hash_password("secure-passphrase").unwrap();
        assert_ne!(a, b);
        assert!(verify_password("secure-passphrase", &a));
        assert!(!verify_password("wrong", &a));
        let legacy = hex::encode(Sha256::digest(b"secure-passphrase"));
        assert!(verify_password("secure-passphrase", &legacy));
        assert!(!verify_password("wrong", &legacy));
        assert!(!verify_password("wrong", "corrupt"));
    }
}
