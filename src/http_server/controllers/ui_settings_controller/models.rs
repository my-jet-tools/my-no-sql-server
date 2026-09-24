use my_http_server::macros::*;
use my_http_server::RawDataTyped;
use serde::{Deserialize, Serialize};

use crate::app::AppContext;

const DEFAULT_WARN_MS: u32 = 3_000;
const DEFAULT_BAD_MS: u32 = 10_000;

/// On-disk shape (`ui-settings.json`). MCP write access is gated by a
/// runtime-only enable window held in `AppContext` — it is never
/// persisted here. See `SettingsPublicModel` for the wire shape
/// returned to UI clients.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UiSettingsModel {
    #[serde(rename = "warnMs")]
    pub warn_ms: u32,
    #[serde(rename = "badMs")]
    pub bad_ms: u32,
}

impl Default for UiSettingsModel {
    fn default() -> Self {
        Self {
            warn_ms: DEFAULT_WARN_MS,
            bad_ms: DEFAULT_BAD_MS,
        }
    }
}

impl UiSettingsModel {
    pub fn sanitized(mut self) -> Self {
        if self.warn_ms > self.bad_ms {
            std::mem::swap(&mut self.warn_ms, &mut self.bad_ms);
        }
        self
    }
}

/// Wire shape returned by GET `/api/Settings`. Combines persisted
/// thresholds with the runtime MCP-writes and UI-writes enable state.
/// `Deserialize` is derived only to satisfy `RawDataTyped`'s bound.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, MyHttpObjectStructure)]
pub struct SettingsPublicModel {
    #[serde(rename = "warnMs")]
    pub warn_ms: u32,
    #[serde(rename = "badMs")]
    pub bad_ms: u32,
    #[serde(rename = "mcpWritesEnabled")]
    pub mcp_writes_enabled: bool,
    #[serde(rename = "mcpWritesRemainingSecs")]
    pub mcp_writes_remaining_secs: Option<u64>,
    #[serde(rename = "uiWritesEnabled")]
    pub ui_writes_enabled: bool,
    #[serde(rename = "uiWritesRemainingSecs")]
    pub ui_writes_remaining_secs: Option<u64>,
}

impl SettingsPublicModel {
    pub fn new(settings: &UiSettingsModel, app: &AppContext) -> Self {
        Self {
            warn_ms: settings.warn_ms,
            bad_ms: settings.bad_ms,
            mcp_writes_enabled: app.is_mcp_write_enabled(),
            mcp_writes_remaining_secs: app.mcp_writes_remaining_secs(),
            ui_writes_enabled: app.is_ui_write_enabled(),
            ui_writes_remaining_secs: app.ui_writes_remaining_secs(),
        }
    }
}

/// Partial-update payload for POST `/api/Settings`. Fields omitted
/// from the body are left as-is on the server.
#[derive(Serialize, Deserialize, Debug, Default, Clone, MyHttpObjectStructure)]
pub struct SettingsPatchBody {
    #[serde(rename = "warnMs", default)]
    pub warn_ms: Option<u32>,
    #[serde(rename = "badMs", default)]
    pub bad_ms: Option<u32>,
}

#[derive(MyHttpInput)]
pub struct SettingsUpdateInput {
    #[http_body_raw(
        description = "Settings JSON. All fields optional: warnMs, badMs. Omitted fields are left unchanged."
    )]
    pub body: RawDataTyped<SettingsPatchBody>,
}

/// Body for POST `/api/Settings/McpWrites`. `enabled: true` opens the
/// 10-minute MCP-writes window; `false` closes it immediately.
#[derive(Serialize, Deserialize, Debug, Default, Clone, MyHttpObjectStructure)]
pub struct McpWritesBody {
    #[serde(rename = "enabled")]
    pub enabled: bool,
}

#[derive(MyHttpInput)]
pub struct McpWritesInput {
    #[http_body_raw(
        description = "JSON body { enabled }. true enables MCP writes for 10 minutes; false disables them immediately."
    )]
    pub body: RawDataTyped<McpWritesBody>,
}

/// Body for POST `/api/Settings/UiWrites`. `enabled: true` opens the
/// 10-minute UI-writes window; `false` closes it immediately.
#[derive(Serialize, Deserialize, Debug, Default, Clone, MyHttpObjectStructure)]
pub struct UiWritesBody {
    #[serde(rename = "enabled")]
    pub enabled: bool,
}

#[derive(MyHttpInput)]
pub struct UiWritesInput {
    #[http_body_raw(
        description = "JSON body { enabled }. true enables UI writes for 10 minutes; false disables them immediately."
    )]
    pub body: RawDataTyped<UiWritesBody>,
}
