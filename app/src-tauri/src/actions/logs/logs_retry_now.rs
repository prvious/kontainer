use tauri::State;
use uuid::Uuid;

use crate::session::log_streams::LogStreams;

/// Skip a reconnecting stream's backoff and try now.
#[tauri::command]
pub fn logs_retry_now(streams: State<'_, LogStreams>, stream_id: Uuid) {
    streams.retry_now(stream_id);
}
