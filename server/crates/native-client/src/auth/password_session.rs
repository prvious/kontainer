//! Shared reply for owner registration and password sign-in.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::SessionCredentials;
use crate::client::{parse_timestamp, parse_uuid};
use crate::credentials::{AccessCredential, RefreshCredential};
use crate::error::NativeClientError;

/// A password-authenticated principal plus its freshly issued session.
#[derive(Debug)]
pub struct PasswordSession {
    /// Controller cluster identity this session belongs to.
    pub cluster_id: String,
    /// Immutable persisted principal identifier.
    pub principal_id: Uuid,
    /// Sign-in email as the owner registered it.
    pub email: String,
    /// Owner display name.
    pub display_name: String,
    /// Freshly issued credentials.
    pub credentials: SessionCredentials,
}

// No `Debug`: this holds plaintext credentials and must never be formatted.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PasswordSessionReply {
    cluster_id: String,
    principal_id: String,
    email: String,
    display_name: String,
    access_credential: String,
    refresh_credential: String,
    access_expires_at: String,
    refresh_expires_at: String,
}

impl PasswordSessionReply {
    pub(crate) fn into_session(self) -> Result<PasswordSession, NativeClientError> {
        Ok(PasswordSession {
            cluster_id: self.cluster_id,
            principal_id: parse_uuid(&self.principal_id)?,
            email: self.email,
            display_name: self.display_name,
            credentials: SessionCredentials {
                access: AccessCredential::parse(&self.access_credential)
                    .map_err(|_| NativeClientError::InvalidResponse)?,
                refresh: RefreshCredential::parse(&self.refresh_credential)
                    .map_err(|_| NativeClientError::InvalidResponse)?,
                access_expires_at: parse_timestamp(&self.access_expires_at)?,
                refresh_expires_at: parse_timestamp(&self.refresh_expires_at)?,
            },
        })
    }
}
