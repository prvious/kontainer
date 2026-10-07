use kontainer_native_client::{LogOpenRequest, ServiceId};
use tauri::{ipc::Channel, AppHandle, State};
use uuid::Uuid;

use crate::session::log_streams::{LogStreamEvent, LogStreams, SessionLogConnector};
use crate::session::manager::InventoryError;

/// Lines of history an open asks for when none is given, and the most it may ask for.
const DEFAULT_TAIL: u32 = 500;
const MAX_TAIL: u32 = 5_000;

/// Follow one service's logs; events arrive on `channel` until the stream ends or is closed.
#[tauri::command]
pub async fn logs_open(
    app: AppHandle,
    streams: State<'_, LogStreams>,
    profile_id: Uuid,
    service_id: String,
    since: Option<i64>,
    tail: Option<u32>,
    channel: Channel<LogStreamEvent>,
) -> Result<Uuid, InventoryError> {
    let service_id = ServiceId::parse(&service_id).map_err(|_| InventoryError::InvalidRequest)?;
    let tail = tail.unwrap_or(DEFAULT_TAIL);
    if tail > MAX_TAIL || since.is_some_and(|since| since < 0) {
        return Err(InventoryError::InvalidRequest);
    }
    let request = LogOpenRequest {
        service_id,
        since_unix_ms: since,
        tail,
    };
    Ok(streams.open(SessionLogConnector { app, profile_id }, request, channel))
}
