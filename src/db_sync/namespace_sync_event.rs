use my_no_sql_sdk::core::db::DbNamespaceName;

use super::SyncEvent;

/// A change together with the namespace it happened in.
///
/// The namespace rides alongside the event instead of inside every one of the
/// `states` structs: readers are routed by `(namespace, table)`, and a table
/// name alone is ambiguous now that two namespaces may each hold a table of the
/// same name.
pub struct NamespaceSyncEvent {
    pub namespace: DbNamespaceName,
    pub event: SyncEvent,
}

impl NamespaceSyncEvent {
    pub fn new(namespace: DbNamespaceName, event: SyncEvent) -> Self {
        Self { namespace, event }
    }
}
