use crate::{config::store, error::AppError};

#[tauri::command]
pub fn save_model_slot_key(slot_id: String, plain: String) -> Result<(), AppError> {
    store::save_model_slot_key(&slot_id, &plain)
}
