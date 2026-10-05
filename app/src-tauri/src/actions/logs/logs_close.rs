use tauri::State;
use uuid::Uuid;

use crate::session::log_streams::LogStreams;

/// Stop one log stream: cancelling its follower closes the socket, which ends the stream on the
/// controller. Closing one that already ended is not an error.
#[tauri::command]
pub fn logs_close(streams: State<'_, LogStreams>, stream_id: Uuid) {
    streams.close(stream_id);
}
