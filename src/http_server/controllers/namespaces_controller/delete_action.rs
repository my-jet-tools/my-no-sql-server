use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::{AppContext, DeleteNamespaceError};

use super::models::DeleteNamespaceContract;

#[http_route(
    method: "DELETE",
    route: "/api/Namespaces",
    controller: "Namespaces",
    input_data: "DeleteNamespaceContract",
    description: "Delete an empty namespace",
    summary: "Deletes a namespace together with its folder on disk. Refused while the namespace still holds tables",
    result:[
        {status_code: 200, description: "Namespace is deleted"},
        {status_code: 400, description: "Namespace is not empty, is the default one, or does not exist"},
    ]
)]
pub struct DeleteNamespaceAction {
    app: Arc<AppContext>,
}

impl DeleteNamespaceAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &DeleteNamespaceAction,
    input_data: DeleteNamespaceContract,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    crate::db_operations::check_app_states(action.app.as_ref())?;

    if let Err(err) = my_no_sql_sdk::validate_namespace_name(input_data.namespace.as_str()) {
        return Err(HttpFailResult::as_forbidden(
            format!("Invalid namespace name. {}", err).into(),
        ));
    }

    // Dropping a namespace which still holds tables would delete those tables
    // too — the caller has to remove them first, so that deleting data is always
    // an explicit act.
    let db_namespace = action
        .app
        .namespaces
        .delete(input_data.namespace.as_str())
        .await
        .map_err(|err| match err {
            DeleteNamespaceError::NotFound => HttpFailResult::as_forbidden(
                format!("Namespace '{}' is not found", input_data.namespace).into(),
            ),
            DeleteNamespaceError::IsDefault => HttpFailResult::as_forbidden(
                "The default namespace can not be deleted"
                    .to_string()
                    .into(),
            ),
            DeleteNamespaceError::NotEmpty(tables_amount) => HttpFailResult::as_forbidden(
                format!(
                    "Namespace '{}' still holds {} table(s). Delete them first",
                    input_data.namespace, tables_amount
                )
                .into(),
            ),
        })?;

    // The namespace is out of the routing tables by now, so nothing can write
    // into it any more and removing its folder is safe. A folder which can not
    // be removed is reported, not swallowed: the namespace is gone from memory
    // either way and the leftover would come back as a namespace on the next
    // start.
    let folder = crate::persist_repo::get_namespace_folder(
        action.app.settings.get_persistence_dest().as_str(),
        db_namespace.name.as_str(),
    );

    if let Err(err) = tokio::fs::remove_dir_all(folder.as_str()).await {
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err(HttpFailResult::as_forbidden(
                format!(
                    "Namespace '{}' is removed from memory but its folder {} can not be deleted. Err: {}",
                    db_namespace.name, folder, err
                )
                .into(),
            ));
        }
    }

    HttpOutput::Empty.into_ok_result(true)
}
