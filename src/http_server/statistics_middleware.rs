use std::sync::Arc;

use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpServerMiddleware};

use crate::app::AppContext;

// Counts traffic into the write statistics, but ONLY for requests we can
// attribute to a known writer — i.e. those that replay the `session` id issued
// during the Ping handshake. For such a request we record:
//   * +1 to write_payloads_per_second (global request count)
//   * +Content-Length to write_bytes_per_second (global payload size)
//   * +1 / +Content-Length to that writer's per-session counters
// Requests without the `session` header are not counted anywhere — we don't
// know who they are. Body length is taken from the `Content-Length` header so
// the body itself is never read here and stays available downstream.
pub struct StatisticsMiddleware {
    app: Arc<AppContext>,
}

impl StatisticsMiddleware {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl HttpServerMiddleware for StatisticsMiddleware {
    async fn handle_request(
        &self,
        ctx: &mut HttpContext,
    ) -> Option<Result<HttpOkResult, HttpFailResult>> {
        // Only count requests that carry the `session` id — i.e. ones we can
        // attribute to a known writer. Everything else is ignored entirely.
        if let Some(session) = get_session(ctx) {
            let body_len = get_content_length(ctx);
            self.app.write_payloads_per_second.increase(1);
            self.app.write_bytes_per_second.increase(body_len);
            self.app.writers_traffic.increase(session, body_len);
        }

        None
    }
}

fn get_session(ctx: &HttpContext) -> Option<&str> {
    use my_http_server::HttpRequestHeaders;

    ctx.request
        .get_headers()
        .try_get_case_insensitive_as_str("session")
        .ok()
        .flatten()
        .filter(|value| !value.is_empty())
}

fn get_content_length(ctx: &HttpContext) -> usize {
    use my_http_server::HttpRequestHeaders;

    ctx.request
        .get_headers()
        .try_get_case_insensitive_as_str("content-length")
        .ok()
        .flatten()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
}
