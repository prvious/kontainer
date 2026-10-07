use tauri::State;
use uuid::Uuid;

use crate::session::manager::{ClusterSummaryView, InventoryError, SessionManager};

/// Node counts by health, quorum, and load summed over online nodes.
#[tauri::command]
pub async fn get_cluster_summary(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
) -> Result<ClusterSummaryView, InventoryError> {
    state.cluster_summary(profile_id).await
}
