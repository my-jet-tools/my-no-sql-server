use std::sync::Arc;

use my_http_server::{HttpContext, HttpFailResult};

use crate::app::{AppContext, DbNamespace};

/// Header naming the namespace a request works in. No header — or an empty one —
/// means the default namespace, which is what every pre-namespace client sends.
///
/// The value is resolved HERE, from the request, for every action — one path,
/// so the query-parameter fallback below applies everywhere. The input
/// contracts additionally declare the header with `#[http_header(name = "ns")]`,
/// but only so that it shows up in the generated OpenAPI schema: swagger is
/// built from the contracts, and a header read straight off the request is
/// invisible to it. Those declared fields are documentation, not the source of
/// truth — reading them instead would quietly skip the query-parameter
/// fallback.
pub const NAMESPACE_HEADER: &str = "ns";

/// Namespace of a request which WRITES.
///
/// The namespace is created if this is the first time it is mentioned, the same
/// way a table is: `ns` is a client-owned name, not something an admin has to
/// register first. Reads must use `get_request_namespace_existing` instead —
/// see there.
pub async fn get_request_namespace(
    app: &Arc<AppContext>,
    ctx: &HttpContext,
) -> Result<Arc<DbNamespace>, HttpFailResult> {
    let result = app
        .get_or_create_namespace(get_request_namespace_name(ctx))
        .await?;

    Ok(result)
}

/// Namespace of a request which must not create one — every READ and every
/// DELETE. An unknown namespace is answered with "namespace not found" straight
/// away instead of being conjured into existence just so the operation can fail
/// inside it.
///
/// A read creating a namespace is not merely untidy, it breaks the backups: the
/// namespace gets a `db/<ns>` folder and lands in the list the backup timer
/// walks, while nothing ever gives it a `backup/<ns>` counterpart. One GET with
/// a mistyped `ns` — one negative test in swagger — was enough to leave that
/// behind, and the folder survives restarts.
pub async fn get_request_namespace_existing(
    app: &Arc<AppContext>,
    ctx: &HttpContext,
) -> Result<Arc<DbNamespace>, HttpFailResult> {
    let result = app.get_existing_namespace(get_request_namespace_name(ctx))?;

    Ok(result)
}

/// Namespace of a reader's subscribe request — neither of the two above, because
/// a subscribe is a read which is allowed to write under one setting, and one
/// which is never refused for naming a namespace that does not exist. See
/// `AppContext::get_namespace_of_subscribe`.
pub async fn get_request_namespace_of_subscribe(
    app: &Arc<AppContext>,
    ctx: &HttpContext,
) -> Result<Option<Arc<DbNamespace>>, HttpFailResult> {
    let result = app
        .get_namespace_of_subscribe(get_request_namespace_name(ctx))
        .await?;

    Ok(result)
}

/// Namespace the request names, if it names one at all. `None` means the default
/// namespace to everything which has to work in exactly one — and "every
/// namespace" to `MakeBackup`, which is the one endpoint that can address them
/// all at once.
pub fn get_request_namespace_name(ctx: &HttpContext) -> Option<&str> {
    match get_namespace_header(ctx) {
        Some(namespace) => Some(namespace),
        None => get_namespace_query_param(ctx),
    }
}

pub fn get_namespace_header(ctx: &HttpContext) -> Option<&str> {
    use my_http_server::HttpRequestHeaders;

    ctx.request
        .get_headers()
        .try_get_case_insensitive_as_str(NAMESPACE_HEADER)
        .ok()
        .flatten()
        .filter(|value| !value.is_empty())
}

/// Fallback for requests which can not carry a header at all: a browser
/// download is an `<a href>`, so the UI has no way to attach `ns` to it. The
/// header wins whenever both are present.
fn get_namespace_query_param(ctx: &HttpContext) -> Option<&str> {
    let query = ctx.request.get_uri().query()?;

    for pair in query.split('&') {
        // A valueless element (`?flag`) is somebody else's parameter, not the
        // end of the query string — keep looking.
        let (key, value) = match pair.split_once('=') {
            Some(result) => result,
            None => continue,
        };

        if key.eq_ignore_ascii_case(NAMESPACE_HEADER) {
            if value.is_empty() {
                return None;
            }

            return Some(value);
        }
    }

    None
}
