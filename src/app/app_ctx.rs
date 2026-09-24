use std::{
    sync::{atomic::AtomicI64, Arc},
    time::Duration,
};

use my_no_sql_sdk::core::rust_extensions::{
    date_time::DateTimeAsMicroseconds, events_loop::EventsLoop, file_utils::FilePath, AppStates,
};

use crate::{
    data_readers::DataReadersList, db_operations::bulk_processes::ActiveBulkProcesses,
    db_operations::multipart::MultipartList, db_sync::NamespaceSyncEvent,
    db_transactions::ActiveTransactions, operations::init::InitState,
    settings_reader::SettingsModel,
};

use super::{
    DbNamespace, DbNamespaces, HttpWriters, OneSecondCounter, PrometheusMetrics, WritersTraffic,
};

pub const APP_VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub const APP_NAME: &'static str = env!("CARGO_PKG_NAME");

pub const DEFAULT_PERSIST_PERIOD: crate::db_sync::DataSynchronizationPeriod =
    crate::db_sync::DataSynchronizationPeriod::Sec5;

/// How long MCP write operations stay enabled after the user clicks
/// "Enable MCP writes" in the UI Settings page.
pub const MCP_WRITES_WINDOW: Duration = Duration::from_secs(600);

/// How long UI write operations stay enabled after the user clicks
/// "Enable" on the Write access card in the UI Settings page. Separate
/// from `MCP_WRITES_WINDOW` on purpose: enabling writes for an MCP agent
/// must not silently open the destructive buttons in the UI, and vice versa.
pub const UI_WRITES_WINDOW: Duration = Duration::from_secs(600);

pub struct AppContext {
    pub created: DateTimeAsMicroseconds,

    /// Every table lives inside a namespace. A request which names no namespace
    /// works in the default one, which is what every client did before
    /// namespaces existed.
    pub namespaces: DbNamespaces,

    pub metrics: PrometheusMetrics,

    pub active_transactions: ActiveTransactions,

    /// Chunked `CleanAndBulkInsert` uploads which are still being collected.
    /// Nothing here is visible to readers until the process is committed.
    pub active_bulk_processes: ActiveBulkProcesses,

    pub data_readers: DataReadersList,

    pub multipart_list: MultipartList,
    //pub persist_io: PersistIoOperations,
    pub init_state: InitState,

    pub settings: Arc<SettingsModel>,
    pub sync: EventsLoop<NamespaceSyncEvent>,
    pub states: Arc<AppStates>,
    /// Serializes `operations::persist` across its entry points (persist timer,
    /// Force-Persist HTTP action, shutdown drain): delete-table cleanup relies
    /// on persist tasks never overlapping. tokio::Mutex because the guard is
    /// held across repo I/O awaits.
    pub persist_call_lock: tokio::sync::Mutex<()>,
    pub http_writers: HttpWriters,

    pub write_payloads_per_second: OneSecondCounter,
    pub write_bytes_per_second: OneSecondCounter,
    pub writers_traffic: WritersTraffic,

    pub use_unix_socket: Option<FilePath>,

    /// Expiry (`unix_microseconds`) of the current MCP-writes enable
    /// window. `0` means MCP writes are disabled. Runtime-only — never
    /// persisted, so a restart always leaves MCP writes disabled.
    mcp_writes_enabled_until: AtomicI64,

    /// Expiry (`unix_microseconds`) of the current UI-writes enable window.
    /// `0` means UI writes are disabled. Runtime-only, same as the MCP one.
    ui_writes_enabled_until: AtomicI64,
}

impl AppContext {
    pub async fn new(settings: Arc<SettingsModel>) -> Self {
        let namespaces = Self::open_namespaces(&settings).await;

        Self {
            created: DateTimeAsMicroseconds::now(),
            namespaces,
            metrics: PrometheusMetrics::new(),
            active_transactions: ActiveTransactions::new(),
            active_bulk_processes: ActiveBulkProcesses::new(),
            states: Arc::new(AppStates::create_un_initialized()),

            data_readers: DataReadersList::new(Duration::from_secs(30)),
            multipart_list: MultipartList::new(),
            persist_call_lock: tokio::sync::Mutex::new(()),
            settings,
            sync: EventsLoop::new("Sync"),
            http_writers: HttpWriters::new(),
            write_payloads_per_second: OneSecondCounter::new(),
            write_bytes_per_second: OneSecondCounter::new(),
            writers_traffic: WritersTraffic::new(),
            init_state: InitState::new(),
            use_unix_socket: match std::env::var("UNIX_SOCKET") {
                Ok(path) => FilePath::from_str(&path).into(),
                Err(_) => None,
            },
            mcp_writes_enabled_until: AtomicI64::new(0),
            ui_writes_enabled_until: AtomicI64::new(0),
        }
    }

    /// Brings up every namespace this server already has on disk, plus the
    /// default one, which exists even when nothing was ever persisted into it.
    async fn open_namespaces(settings: &Arc<SettingsModel>) -> DbNamespaces {
        let namespaces = DbNamespaces::new(settings.clone());

        let root = settings.get_persistence_dest();

        crate::persist_repo::migrate_legacy_root_into_default_namespace(root.as_str()).await;

        for namespace in crate::persist_repo::get_namespaces_on_disk(root.as_str()).await {
            namespaces.get_or_create(namespace.as_str()).await;
        }

        namespaces
            .get_or_create(my_no_sql_sdk::DEFAULT_NAMESPACE)
            .await;

        namespaces
    }

    /// Backups are laid out per namespace as well, so a pre-namespace backup
    /// folder has to be carried into `default/` the same way the persistence
    /// root is.
    pub async fn migrate_backup_folder(&self) {
        crate::operations::backup::migrate_legacy_backup_folder(self).await;
    }

    /// Namespace a write works in: the one it names, or the default one when it
    /// names none. A namespace which does not exist yet is created on the spot —
    /// the same way a table is.
    pub async fn get_or_create_namespace(
        &self,
        name: Option<&str>,
    ) -> Result<Arc<DbNamespace>, crate::db_operations::DbOperationError> {
        match Self::parse_namespace_name(name)? {
            Some(name) => Ok(self.namespaces.get_or_create(name).await),
            None => Ok(self.namespaces.get_default()),
        }
    }

    /// Namespace a reader's `Subscribe` works in. `None` means it does not exist
    /// — which is NOT an error here: a reader naming a namespace nobody has
    /// written to yet is answered with an empty snapshot, see
    /// `operations::data_readers::send_empty_snapshot`. A malformed name still
    /// is one.
    ///
    /// A subscribe reads — unless `AutoCreateTableOnReaderSubscribe` is on, in
    /// which case it is allowed to create the table it subscribes to, and then it
    /// has to be allowed to create the namespace that table would live in as
    /// well.
    pub async fn get_namespace_of_subscribe(
        &self,
        name: Option<&str>,
    ) -> Result<Option<Arc<DbNamespace>>, crate::db_operations::DbOperationError> {
        if self.settings.auto_create_table_on_reader_subscribe {
            return Ok(Some(self.get_or_create_namespace(name).await?));
        }

        match self.get_existing_namespace(name) {
            Ok(db_namespace) => Ok(Some(db_namespace)),
            Err(crate::db_operations::DbOperationError::NamespaceNotFound(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Namespace of an operation which must not bring one into existence —
    /// everything which does not ADD data: reads, deletes, table cleanups.
    ///
    /// Such an operation has nothing to work on in a namespace nobody ever wrote
    /// to, and resolving it the creating way leaves a folder on disk behind every
    /// mistyped request — a folder the backup timer then expects a
    /// `backup/<ns>` counterpart for. The default namespace always exists.
    pub fn get_existing_namespace(
        &self,
        name: Option<&str>,
    ) -> Result<Arc<DbNamespace>, crate::db_operations::DbOperationError> {
        let name = match Self::parse_namespace_name(name)? {
            Some(name) => name,
            None => return Ok(self.namespaces.get_default()),
        };

        match self.namespaces.get(name) {
            Some(db_namespace) => Ok(db_namespace),
            None => Err(crate::db_operations::DbOperationError::NamespaceNotFound(
                name.to_string(),
            )),
        }
    }

    /// `None` means "the default namespace": both a missing and an empty name
    /// are what every pre-namespace client sends.
    /// Returns a BORROWED name: a lookup needs no owned `DbNamespaceName`, and
    /// building one here would allocate on every request that names a namespace.
    fn parse_namespace_name(
        name: Option<&str>,
    ) -> Result<Option<&str>, crate::db_operations::DbOperationError> {
        let name = match name {
            Some(name) => name.trim(),
            None => return Ok(None),
        };

        if name.is_empty() {
            return Ok(None);
        }

        if let Err(err) = my_no_sql_sdk::validate_namespace_name(name) {
            return Err(
                crate::db_operations::DbOperationError::NamespaceNameValidationError(
                    err.to_string(),
                ),
            );
        }

        if name == my_no_sql_sdk::DEFAULT_NAMESPACE {
            return Ok(None);
        }

        Ok(Some(name))
    }

    /// Enables MCP write operations for `MCP_WRITES_WINDOW`.
    pub fn enable_mcp_writes(&self) {
        Self::extend_write_window(&self.mcp_writes_enabled_until, MCP_WRITES_WINDOW);
    }

    /// Adds `window` to what is LEFT of the current window instead of resetting
    /// it to `window`. The UI offers this as an explicit "Extend +10 min"
    /// button, and pressing it must never be able to shorten a window that is
    /// already open — which a plain reset does whenever more than a moment of
    /// the previous window remains.
    ///
    /// `fetch_update` rather than load-then-store: two people pressing the
    /// button at the same time should both add their ten minutes, not have one
    /// of them silently dropped.
    fn extend_write_window(slot: &AtomicI64, window: Duration) {
        let now = DateTimeAsMicroseconds::now();

        let _ = slot.fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |current| {
                // An expired window (or one that was never opened) starts
                // counting from now; a live one keeps its remainder and grows.
                let base = if current > now.unix_microseconds {
                    DateTimeAsMicroseconds::new(current)
                } else {
                    now
                };

                Some(base.add(window).unix_microseconds)
            },
        );
    }

    /// Disables MCP write operations immediately.
    pub fn disable_mcp_writes(&self) {
        self.mcp_writes_enabled_until
            .store(0, std::sync::atomic::Ordering::SeqCst);
    }

    /// `Some(expiry)` while the enable window is still open, otherwise `None`.
    pub fn mcp_writes_enabled_until(&self) -> Option<DateTimeAsMicroseconds> {
        let until = self
            .mcp_writes_enabled_until
            .load(std::sync::atomic::Ordering::Relaxed);
        if until > DateTimeAsMicroseconds::now().unix_microseconds {
            Some(DateTimeAsMicroseconds::new(until))
        } else {
            None
        }
    }

    pub fn is_mcp_write_enabled(&self) -> bool {
        self.mcp_writes_enabled_until().is_some()
    }

    /// Seconds left in the enable window, or `None` when disabled.
    pub fn mcp_writes_remaining_secs(&self) -> Option<u64> {
        let until = self.mcp_writes_enabled_until()?;
        let now = DateTimeAsMicroseconds::now();
        let micros_left = until.unix_microseconds - now.unix_microseconds;
        Some((micros_left.max(0) / 1_000_000) as u64)
    }

    /// Enables UI write operations for `UI_WRITES_WINDOW`.
    pub fn enable_ui_writes(&self) {
        Self::extend_write_window(&self.ui_writes_enabled_until, UI_WRITES_WINDOW);
    }

    /// Disables UI write operations immediately.
    pub fn disable_ui_writes(&self) {
        self.ui_writes_enabled_until
            .store(0, std::sync::atomic::Ordering::SeqCst);
    }

    /// `Some(expiry)` while the enable window is still open, otherwise `None`.
    pub fn ui_writes_enabled_until(&self) -> Option<DateTimeAsMicroseconds> {
        let until = self
            .ui_writes_enabled_until
            .load(std::sync::atomic::Ordering::Relaxed);
        if until > DateTimeAsMicroseconds::now().unix_microseconds {
            Some(DateTimeAsMicroseconds::new(until))
        } else {
            None
        }
    }

    pub fn is_ui_write_enabled(&self) -> bool {
        self.ui_writes_enabled_until().is_some()
    }

    /// Seconds left in the enable window, or `None` when disabled.
    pub fn ui_writes_remaining_secs(&self) -> Option<u64> {
        let until = self.ui_writes_enabled_until()?;
        let now = DateTimeAsMicroseconds::now();
        let micros_left = until.unix_microseconds - now.unix_microseconds;
        Some((micros_left.max(0) / 1_000_000) as u64)
    }
}
