use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, SampleSeriesView, SessionManager};

/// Cluster-wide bucketed samples for one window.
#[tauri::command]
pub async fn get_cluster_samples(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    window: String,
    bucket: String,
) -> Result<SampleSeriesView, InventoryError> {
    state.cluster_samples(profile_id, &window, &bucket).await
}
