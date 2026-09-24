use std::sync::Arc;

use arc_swap::ArcSwap;

use my_no_sql_sdk::core::db::DbNamespaceName;
use my_no_sql_sdk::server::DbInstance;

use crate::persist_markers::PersistMarkers;
use crate::persist_repo::PersistRepo;
use crate::settings_reader::SettingsModel;

/// Everything which belongs to a single namespace: its tables, its persistence
/// backend and its persist queue. Namespaces share nothing — a table of one
/// namespace is invisible to every other one, and each namespace persists into
/// a folder of its own (`<root>/default`, `<root>/alpha`, …).
pub struct DbNamespace {
    pub name: DbNamespaceName,
    pub db: DbInstance,
    pub repo: PersistRepo,
    pub persist_markers: PersistMarkers,
}

impl DbNamespace {
    pub async fn open(name: DbNamespaceName, settings: &SettingsModel) -> Self {
        let repo = settings.open_persist_repo(name.as_str()).await;

        Self {
            name,
            db: DbInstance::new(),
            repo,
            persist_markers: PersistMarkers::new(),
        }
    }

    pub fn tables_amount(&self) -> usize {
        self.db.get_tables().len()
    }
}

#[derive(Debug)]
pub enum DeleteNamespaceError {
    NotFound,
    IsDefault,
    /// The namespace still holds tables — deleting it would delete them too, so
    /// the caller has to empty it first.
    NotEmpty(usize),
}

/// All the namespaces of this server. A namespace materializes lazily: the first
/// write or the first subscription which mentions it brings it to life together
/// with its folder on disk.
pub struct DbNamespaces {
    /// Copy-on-write behind an `ArcSwap`, holding a plain `Vec`.
    ///
    /// A `Vec` and not a map because a deployment runs one or two namespaces and
    /// five at the very most: a linear scan over a contiguous slice beats
    /// hashing or a tree descent at that size, and this is looked up on EVERY
    /// request and every TCP packet, so the constant is what matters, not the
    /// asymptotics.
    ///
    /// `ArcSwap` and not a lock because namespaces are created about as often as
    /// the server is deployed, while they are READ constantly. A reader pays a
    /// single atomic load and no lock at all; a writer rebuilds the whole list
    /// and swaps it in.
    items: ArcSwap<Vec<Arc<DbNamespace>>>,
    /// Serializes the writers among themselves — `ArcSwap` makes the swap
    /// atomic but not the read-modify-write around it. Also covers the async
    /// part: opening a persistence backend awaits, and without this two callers
    /// racing on the same new namespace would open its folder twice.
    write_lock: tokio::sync::Mutex<()>,
    settings: Arc<SettingsModel>,
}

impl DbNamespaces {
    pub fn new(settings: Arc<SettingsModel>) -> Self {
        Self {
            items: ArcSwap::from_pointee(Vec::new()),
            write_lock: tokio::sync::Mutex::new(()),
            settings,
        }
    }

    /// Takes a `&str` rather than a `DbNamespaceName`: the name arrives as a
    /// slice of a header, and building a `DbNamespaceName` to look it up would
    /// allocate an `Arc<String>` on every request just to throw it away.
    pub fn get(&self, name: &str) -> Option<Arc<DbNamespace>> {
        self.items
            .load()
            .iter()
            .find(|itm| itm.name.as_str() == name)
            .cloned()
    }

    /// The hot path — most requests name no namespace at all. Scans for the
    /// default one instead of constructing `DbNamespaceName::default()`, which
    /// would allocate a `String` and an `Arc` on every single request.
    pub fn get_default(&self) -> Arc<DbNamespace> {
        self.items
            .load()
            .iter()
            .find(|itm| itm.name.is_default())
            .cloned()
            .expect("Default namespace must be created during the app start up")
    }

    /// Returns the namespace, creating it (and its folder on disk) if this is the
    /// first time it is mentioned.
    pub async fn get_or_create(&self, name: &str) -> Arc<DbNamespace> {
        if let Some(result) = self.get(name) {
            return result;
        }

        let _write_lock = self.write_lock.lock().await;

        // Somebody could have created it while we were waiting for the lock.
        if let Some(result) = self.get(name) {
            return result;
        }

        // Only here, on the rare creation path, is the owned name worth building.
        let namespace = Arc::new(DbNamespace::open(name.into(), self.settings.as_ref()).await);

        let mut new_items = self.items.load().as_ref().clone();
        new_items.push(namespace.clone());
        self.items.store(Arc::new(new_items));

        namespace
    }

    pub fn get_all(&self) -> Vec<Arc<DbNamespace>> {
        self.items.load().as_ref().clone()
    }

    /// Removes an empty namespace. The emptiness check and the removal happen
    /// under the same write lock — otherwise a table created in between would be
    /// dropped from memory together with the namespace.
    pub async fn delete(&self, name: &str) -> Result<Arc<DbNamespace>, DeleteNamespaceError> {
        let _write_lock = self.write_lock.lock().await;

        let mut new_items = self.items.load().as_ref().clone();

        let index = match new_items.iter().position(|itm| itm.name.as_str() == name) {
            Some(index) => index,
            None => return Err(DeleteNamespaceError::NotFound),
        };

        if new_items[index].name.is_default() {
            return Err(DeleteNamespaceError::IsDefault);
        }

        let tables_amount = new_items[index].tables_amount();

        if tables_amount > 0 {
            return Err(DeleteNamespaceError::NotEmpty(tables_amount));
        }

        let removed = new_items.remove(index);
        self.items.store(Arc::new(new_items));

        Ok(removed)
    }
}
