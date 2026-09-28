use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, SampleSeriesView, SessionManager};

/// One node's bucketed samples for one window.
#[tauri::command]
pub async fn get_node_samples(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    node_id: String,
    window: String,
    bucket: String,
) -> Result<SampleSeriesView, InventoryError> {
    state
        .samples(profile_id, Some(&node_id), &window, &bucket)
        .await
}
