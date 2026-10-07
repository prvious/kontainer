use tauri::State;
use uuid::Uuid;

use crate::session::manager::{ImageListView, InventoryError, SessionManager};

/// Images on every answering node, merged by id, with one outcome per member.
#[tauri::command]
pub async fn list_images(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
) -> Result<ImageListView, InventoryError> {
    state.images(profile_id).await
}
