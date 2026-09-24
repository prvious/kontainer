//! OS credential-vault access: one fixed anchor plus UUID-keyed profile
//! records.
//!
//! Every platform-store call runs on one dedicated worker thread. The worker
//! serializes access (Windows Credential Manager does not order concurrent
//! calls reliably), keeps potentially prompting calls off the async runtime,
//! and zeroes its copy of any secret buffer after use. Callers await a oneshot
//! reply; dropping a call never detaches work, because the worker completes
//! each request before reading the next.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::thread;

use keyring_core::{CredentialStore, Entry, Error as KeyringError};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

/// Service name every desktop credential is filed under.
pub(crate) const VAULT_SERVICE: &str = "dev.kontainer.app";
/// Fixed user name of the trusted anchor entry.
pub(crate) const ANCHOR_USER: &str = "active-profile-anchor";

/// Boxed future used by the object-safe vault trait.
pub(crate) type VaultFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Closed vault failure taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VaultError {
    /// The platform store is missing, locked, or denied access.
    Unavailable,
    /// The platform store rejected a write.
    Write,
    /// The platform store rejected a delete.
    Delete,
}

/// Keyed access to the OS credential store.
///
/// `read_anchor` addresses the one fixed, integrity-bearing record; profile
/// records are addressed only by the UUID that record supplied. Read payloads
/// are returned in a [`Zeroizing`] buffer that clears its owned bytes on drop.
pub(crate) trait CredentialVault: Send + Sync + 'static {
    fn read_anchor(&self) -> VaultFuture<'_, Result<Option<Zeroizing<Vec<u8>>>, VaultError>>;
    fn write_anchor(&self, payload: Vec<u8>) -> VaultFuture<'_, Result<(), VaultError>>;
    fn read_profile(
        &self,
        profile_id: Uuid,
    ) -> VaultFuture<'_, Result<Option<Zeroizing<Vec<u8>>>, VaultError>>;
    fn write_profile<'a>(
        &'a self,
        profile_id: Uuid,
        payload: &'a [u8],
    ) -> VaultFuture<'a, Result<(), VaultError>>;
    fn delete_profile(&self, profile_id: Uuid) -> VaultFuture<'_, Result<(), VaultError>>;
}

enum VaultRequest {
    ReadAnchor(oneshot::Sender<Result<Option<Zeroizing<Vec<u8>>>, VaultError>>),
    WriteAnchor(Vec<u8>, oneshot::Sender<Result<(), VaultError>>),
    ReadProfile(
        Uuid,
        oneshot::Sender<Result<Option<Zeroizing<Vec<u8>>>, VaultError>>,
    ),
    WriteProfile(Uuid, Vec<u8>, oneshot::Sender<Result<(), VaultError>>),
    DeleteProfile(Uuid, oneshot::Sender<Result<(), VaultError>>),
}

/// Production vault backed by the platform credential store.
pub(crate) struct KeyringVault {
    requests: mpsc::UnboundedSender<VaultRequest>,
}

impl KeyringVault {
    /// Start the worker thread. A platform store that cannot be opened makes
    /// every later call fail closed with [`VaultError::Unavailable`].
    pub(crate) fn spawn() -> Self {
        let (requests, receiver) = mpsc::unbounded_channel();
        // A failed thread spawn closes the receiver, so all calls fail closed
        // through the ordinary send-error path.
        let _ = thread::Builder::new()
            .name("kontainer-session-vault".to_owned())
            .spawn(move || worker_loop(receiver));
        Self { requests }
    }

    fn request<T: Send + 'static>(
        &self,
        build: impl FnOnce(oneshot::Sender<Result<T, VaultError>>) -> VaultRequest,
    ) -> VaultFuture<'_, Result<T, VaultError>> {
        let (reply, wait) = oneshot::channel();
        if let Err(send_error) = self.requests.send(build(reply)) {
            // A worker that is gone cannot own the plaintext payload.
            zeroize_request(send_error.0);
            return Box::pin(async { Err(VaultError::Unavailable) });
        }
        Box::pin(async move { wait.await.unwrap_or(Err(VaultError::Unavailable)) })
    }
}

/// Zero the secret payload of a request that could never be delivered.
fn zeroize_request(request: VaultRequest) {
    match request {
        VaultRequest::WriteAnchor(mut payload, _) => payload.zeroize(),
        VaultRequest::WriteProfile(_, mut payload, _) => payload.zeroize(),
        VaultRequest::ReadAnchor(_)
        | VaultRequest::ReadProfile(_, _)
        | VaultRequest::DeleteProfile(_, _) => {}
    }
}

impl CredentialVault for KeyringVault {
    fn read_anchor(&self) -> VaultFuture<'_, Result<Option<Zeroizing<Vec<u8>>>, VaultError>> {
        self.request(VaultRequest::ReadAnchor)
    }

    fn write_anchor(&self, payload: Vec<u8>) -> VaultFuture<'_, Result<(), VaultError>> {
        self.request(move |reply| VaultRequest::WriteAnchor(payload, reply))
    }

    fn read_profile(
        &self,
        profile_id: Uuid,
    ) -> VaultFuture<'_, Result<Option<Zeroizing<Vec<u8>>>, VaultError>> {
        self.request(move |reply| VaultRequest::ReadProfile(profile_id, reply))
    }

    fn write_profile<'a>(
        &'a self,
        profile_id: Uuid,
        payload: &'a [u8],
    ) -> VaultFuture<'a, Result<(), VaultError>> {
        let payload = payload.to_vec();
        self.request(move |reply| VaultRequest::WriteProfile(profile_id, payload, reply))
    }

    fn delete_profile(&self, profile_id: Uuid) -> VaultFuture<'_, Result<(), VaultError>> {
        self.request(move |reply| VaultRequest::DeleteProfile(profile_id, reply))
    }
}

fn worker_loop(mut receiver: mpsc::UnboundedReceiver<VaultRequest>) {
    let store = build_platform_store();
    while let Some(request) = receiver.blocking_recv() {
        match request {
            VaultRequest::ReadAnchor(reply) => {
                let _ = reply.send(read_secret(&store, ANCHOR_USER));
            }
            VaultRequest::WriteAnchor(mut payload, reply) => {
                let result = write_secret(&store, ANCHOR_USER, &payload);
                payload.zeroize();
                let _ = reply.send(result);
            }
            VaultRequest::ReadProfile(profile_id, reply) => {
                let _ = reply.send(read_secret(&store, &profile_user(profile_id)));
            }
            VaultRequest::WriteProfile(profile_id, mut payload, reply) => {
                let result = write_secret(&store, &profile_user(profile_id), &payload);
                payload.zeroize();
                let _ = reply.send(result);
            }
            VaultRequest::DeleteProfile(profile_id, reply) => {
                let _ = reply.send(delete_secret(&store, &profile_user(profile_id)));
            }
        }
    }
}

fn profile_user(profile_id: Uuid) -> String {
    format!("profile:{profile_id}")
}

/// Read one entry. Unavailability versus a genuinely absent entry must stay
/// distinguishable: the caller may only treat `Ok(None)` as "no stored
/// credential".
fn read_secret(
    store: &Result<Arc<CredentialStore>, KeyringError>,
    user: &str,
) -> Result<Option<Zeroizing<Vec<u8>>>, VaultError> {
    let entry = entry_for(store, user)?;
    match entry.get_secret() {
        Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(_) => Err(VaultError::Unavailable),
    }
}

fn write_secret(
    store: &Result<Arc<CredentialStore>, KeyringError>,
    user: &str,
    payload: &[u8],
) -> Result<(), VaultError> {
    entry_for(store, user)?
        .set_secret(payload)
        .map_err(|_| VaultError::Write)
}

fn delete_secret(
    store: &Result<Arc<CredentialStore>, KeyringError>,
    user: &str,
) -> Result<(), VaultError> {
    match entry_for(store, user)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(_) => Err(VaultError::Delete),
    }
}

fn entry_for(
    store: &Result<Arc<CredentialStore>, KeyringError>,
    user: &str,
) -> Result<Entry, VaultError> {
    let store = store.as_ref().map_err(|_| VaultError::Unavailable)?;
    build_entry(store, user).map_err(|_| VaultError::Unavailable)
}

/// Build one entry through the same path every vault operation uses.
fn build_entry(store: &Arc<CredentialStore>, user: &str) -> Result<Entry, KeyringError> {
    // Windows Credential Manager defaults to enterprise persistence; a desktop
    // session credential is only meaningful for this machine and user.
    #[cfg(target_os = "windows")]
    let modifiers = std::collections::HashMap::from([("persistence", "Local")]);
    #[cfg(target_os = "windows")]
    let modifiers = Some(&modifiers);
    #[cfg(not(target_os = "windows"))]
    let modifiers = None;
    store.build(VAULT_SERVICE, user, modifiers)
}

#[cfg(target_os = "macos")]
fn build_platform_store() -> Result<Arc<CredentialStore>, KeyringError> {
    // The desktop app is not sandboxed, so the legacy keychain store is the
    // correct provider.
    Ok(apple_native_keyring_store::keychain::Store::new()?)
}

#[cfg(target_os = "linux")]
fn build_platform_store() -> Result<Arc<CredentialStore>, KeyringError> {
    Ok(zbus_secret_service_keyring_store::Store::new()?)
}

#[cfg(target_os = "windows")]
fn build_platform_store() -> Result<Arc<CredentialStore>, KeyringError> {
    Ok(windows_native_keyring_store::Store::new()?)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn build_platform_store() -> Result<Arc<CredentialStore>, KeyringError> {
    Err(KeyringError::NotSupportedByStore(
        "no native credential store is supported on this platform".to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opt-in round trip against the real platform store. Kept under an
    /// ignored test so ordinary suites never touch the operator's vault; the
    /// desktop qualification workflow runs it on every supported OS.
    #[test]
    #[ignore = "requires a real, unlocked OS credential store"]
    fn real_vault_round_trip() {
        // Randomized so a crashed earlier run can never fail a later one.
        let user = format!("vault-smoke-entry-{}", Uuid::new_v4());
        let store = build_platform_store().expect("platform store builds");

        struct Cleanup(Arc<CredentialStore>, String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                if let Ok(entry) = self.0.build(VAULT_SERVICE, &self.1, None) {
                    let _ = entry.delete_credential();
                }
            }
        }
        let _guard = Cleanup(store.clone(), user.clone());

        // The smoke entry uses the same service and builder path as
        // production use, only a distinct user name.
        let entry = build_entry(&store, &user).expect("entry builds");
        assert!(matches!(entry.get_secret(), Err(KeyringError::NoEntry)));
        entry.set_secret(b"first").expect("initial write");
        assert_eq!(entry.get_secret().expect("read"), b"first");
        entry.set_secret(b"second").expect("replacement write");
        assert_eq!(entry.get_secret().expect("re-read"), b"second");

        // The Windows provider must not fall back to enterprise persistence.
        #[cfg(target_os = "windows")]
        {
            let attributes = entry.get_attributes().expect("attributes");
            assert_eq!(
                attributes.get("persistence").map(String::as_str),
                Some("Local")
            );
        }

        entry.delete_credential().expect("delete");
        assert!(matches!(entry.get_secret(), Err(KeyringError::NoEntry)));
        // Deletion is idempotent at the vault layer.
        assert!(delete_secret(&Ok(store), &user).is_ok());
    }
}
