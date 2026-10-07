use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, SessionManager, StackPlanView};

/// What removing the stack takes away and the volumes it keeps.
#[tauri::command]
pub async fn get_stack_removal_plan(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    name: String,
) -> Result<StackPlanView, InventoryError> {
    state.stack_plan(profile_id, &name).await
}
