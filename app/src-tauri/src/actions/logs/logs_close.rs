use tauri::State;
use uuid::Uuid;

use crate::session::log_streams::LogStreams;

/// Stop one log stream; closing one that already ended is not an error.
#[tauri::command]
pub fn logs_close(streams: State<'_, LogStreams>, stream_id: Uuid) {
    streams.close(stream_id);
}
