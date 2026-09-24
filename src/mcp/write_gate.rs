use crate::app::AppContext;

/// Gate for the MCP writer tools. MCP writes must be explicitly enabled
/// by the admin on the UI Settings page; the enable window lasts 10
/// minutes (or until the admin disables it). Returns a tool-friendly
/// error string when writes are currently disabled.
pub fn ensure_mcp_writes_enabled(app: &AppContext) -> Result<(), String> {
    if app.is_mcp_write_enabled() {
        return Ok(());
    }

    Err(
        "MCP write operations are currently DISABLED. Ask the user to open the \
         MyNoSqlServer UI \u{2192} Settings and click \"Enable MCP writes\" (writes stay on \
         for 10 minutes, or until the user disables them). Do not retry until enabled."
            .into(),
    )
}
