use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use log;

/// Session data for BRC-103/104 authentication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthSession {
    /// The nonce we generated and sent to the app
    pub our_nonce: String,
    /// The app's identity key
    pub their_identity_key: String,
    /// Unix timestamp when session was created
    pub created_at: u64,
    /// Unix timestamp when session expires (24 hours default)
    pub expires_at: u64,
}

impl AuthSession {
    pub fn new(our_nonce: String, their_identity_key: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        AuthSession {
            our_nonce,
            their_identity_key,
            created_at: now,
            expires_at: now + (24 * 60 * 60), // 24 hours
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now > self.expires_at
    }
}

/// Manages authentication sessions for BRC-103/104
pub struct AuthSessionManager {
    // Maps "identity_key:our_nonce" -> AuthSession (composite key for multiple concurrent sessions)
    sessions: Mutex<HashMap<String, AuthSession>>,
}

impl AuthSessionManager {
    pub fn new() -> Self {
        AuthSessionManager {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Create a composite session key from identity and nonce
    fn make_session_key(identity_key: &str, our_nonce: &str) -> String {
        format!("{}:{}", identity_key, our_nonce)
    }

    /// Store a new auth session
    pub fn store_session(&self, identity_key: &str, our_nonce: &str) {
        let mut sessions = self.sessions.lock().unwrap();

        let session = AuthSession::new(
            our_nonce.to_string(),
            identity_key.to_string(),
        );

        let session_key = Self::make_session_key(identity_key, our_nonce);

        log::info!("💾 Storing auth session for {}", identity_key);
        log::info!("   Our nonce: {}", our_nonce);
        log::info!("   Session key: {}", session_key);

        sessions.insert(session_key, session);

        // Clean up expired sessions
        self.cleanup_expired_sessions(&mut sessions);
    }

    /// Retrieve an auth session by identity key and nonce
    pub fn get_session(&self, identity_key: &str, our_nonce: &str) -> Option<AuthSession> {
        let sessions = self.sessions.lock().unwrap();
        let session_key = Self::make_session_key(identity_key, our_nonce);

        if let Some(session) = sessions.get(&session_key) {
            if session.is_expired() {
                log::warn!("⚠️  Session {} has expired", session_key);
                return None;
            }

            log::info!("✅ Retrieved session for {}", identity_key);
            log::info!("   Our nonce: {}", session.our_nonce);
            return Some(session.clone());
        }

        log::warn!("⚠️  No session found for key: {}", session_key);
        None
    }

    /// Check if a session exists and is valid
    pub fn has_valid_session(&self, identity_key: &str, our_nonce: &str) -> bool {
        self.get_session(identity_key, our_nonce).is_some()
    }

    /// Remove a specific session
    pub fn remove_session(&self, identity_key: &str, our_nonce: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        let session_key = Self::make_session_key(identity_key, our_nonce);
        if sessions.remove(&session_key).is_some() {
            log::info!("🗑️  Removed session: {}", session_key);
        }
    }

    /// Clean up expired sessions
    fn cleanup_expired_sessions(&self, sessions: &mut HashMap<String, AuthSession>) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expired: Vec<String> = sessions
            .iter()
            .filter(|(_, session)| now > session.expires_at)
            .map(|(key, _)| key.clone())
            .collect();

        for key in expired {
            sessions.remove(&key);
            log::info!("🗑️  Cleaned up expired session for {}", key);
        }
    }

    /// Get count of active sessions
    pub fn session_count(&self) -> usize {
        let sessions = self.sessions.lock().unwrap();
        sessions.len()
    }
}
