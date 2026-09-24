use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult};
use std::sync::Arc;

use crate::{app::AppContext, http_server::controllers::row_controller::models::BaseDbRowContract};

use super::models::NextMultipartRequestInputContract;

#[http_route(
    method: "POST",
    route: "/api/Multipart/Next",
    deprecated_routes: ["/Multipart/Next"],
    controller: "Multipart",
    description: "New multipart request is started",
    summary: "Returns first multipart amount of rows",
    input_data: "NextMultipartRequestInputContract",
    result:[
        {status_code: 200, description: "Chunk of entities", model: "Vec<BaseDbRowContract>" },
    ]
)]
pub struct NextMultipartAction {
    app: Arc<AppContext>,
}

impl NextMultipartAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
/*
#[async_trait::async_trait]
impl PostAction for NextMultipartController {
    fn get_route(&self) -> &str {
        "/Multipart/Next"
    }

    fn get_description(&self) -> Option<HttpActionDescription> {
        HttpActionDescription {
            controller_name: super::consts::CONTROLLER_NAME,
            description: "Monitoring API",

            input_params: Some(NextMultipartRequestInputContract::get_input_params()),
            results: vec![
                BaseDbRowContract::get_http_data_structure().into_http_result_array(
                    200,
                    false,
                    "Chunk of entities",
                ),
            ],
        }
        .into()
    }
    async fn handle_request(&self, ctx: &mut HttpContext) -> Result<HttpOkResult, HttpFailResult> {
        let input_data = NextMultipartRequestInputContract::parse_http_input(ctx).await?;

        let db_rows = crate::db_operations::read::multipart::get_next(
            self.app.as_ref(),
            input_data.request_id,
            input_data.max_records_count,
        )
        .await;

        if db_rows.is_none() {
            return Err(HttpFailResult::as_not_found("".to_string(), false));
        }

        return Ok(db_rows.unwrap().into());
    }
}
*/

async fn handle_request(
    action: &NextMultipartAction,
    input_data: NextMultipartRequestInputContract,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_rows = crate::db_operations::read::multipart::get_next(
        action.app.as_ref(),
        input_data.request_id,
        input_data.max_records_count,
    )
    .await;

    if db_rows.is_none() {
        return Err(HttpFailResult::as_not_found("".to_string(), false));
    }

    return Ok(db_rows.unwrap().into());
}
