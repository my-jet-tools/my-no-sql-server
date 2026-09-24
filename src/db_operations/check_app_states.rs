use crate::app::AppContext;

use super::DbOperationError;

pub fn check_app_states(app: &AppContext) -> Result<(), DbOperationError> {
    if app.states.is_initialized() {
        return Ok(());
    }

    Err(DbOperationError::ApplicationIsNotInitializedYet)
}
