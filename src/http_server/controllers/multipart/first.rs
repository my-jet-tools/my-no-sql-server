use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

use super::models::{NewMultipartInputContract, NewMultipartResponse};

#[http_route(
    method: "POST",
    route: "/api/Multipart/First",
    deprecated_routes: ["/Multipart/First"],
    controller: "Multipart",
    description: "New multipart request is started",
    summary: "Returns first multipart amount of rows",
    input_data: "NewMultipartInputContract",
    result:[
        {status_code: 200, description: "Rows"},
    ]
)]
pub struct FirstMultipartAction {
    app: Arc<AppContext>,
}

impl FirstMultipartAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

/*
#[async_trait::async_trait]
impl PostAction for FirstMultipartController {
    fn get_route(&self) -> &str {
        "/Multipart/First"
    }

    fn get_description(&self) -> Option<HttpActionDescription> {
        HttpActionDescription {
            controller_name: super::consts::CONTROLLER_NAME,
            description: "Monitoring API",

            input_params: Some(NewMultipartInputContract::get_input_params()),
            results: vec![
                NewMultipartResponse::get_http_data_structure().into_http_result_object(
                    200,
                    false,
                    "New multipart is started",
                ),
            ],
        }
        .into()
    }
    async fn handle_request(&self, ctx: &mut HttpContext) -> Result<HttpOkResult, HttpFailResult> {
        let input_data = NewMultipartInputContract::parse_http_input(ctx).await?;

        let result = crate::db_operations::read::multipart::start_read_all(
            self.app.as_ref(), &db_namespace,
            input_data.table_name.as_ref(),
        )
        .await?;

        let response = NewMultipartResponse {
            snapshot_id: format!("{}", result),
        };

        HttpOutput::as_json(response).into_ok_result(true).into()
    }
}
 */

async fn handle_request(
    action: &FirstMultipartAction,
    input_data: NewMultipartInputContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let result = crate::db_operations::read::multipart::start_read_all(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_ref(),
    )
    .await?;

    let response = NewMultipartResponse {
        snapshot_id: format!("{}", result),
    };

    HttpOutput::as_json(response).into_ok_result(true).into()
}
