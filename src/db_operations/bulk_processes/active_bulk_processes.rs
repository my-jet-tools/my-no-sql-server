use std::{collections::BTreeMap, sync::Arc, time::Duration};

use my_no_sql_sdk::core::db::{DbNamespaceName, DbRow};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::sync::Mutex;

use super::{BulkProcess, BulkProcessError, BulkProcessScope};

pub struct ActiveBulkProcesses {
    items: Mutex<BTreeMap<String, BulkProcess>>,
}

impl ActiveBulkProcesses {
    pub fn new() -> Self {
        Self {
            items: Mutex::new(BTreeMap::new()),
        }
    }

    /// Starts a new process and puts the first chunk into it.
    ///
    /// A writer holds at most one process per table: if the session already has
    /// a process for the same table it is dropped here. That is the recovery
    /// path - a writer whose upload died half-way just starts over instead of
    /// leaving a staging snapshot hanging around until GC.
    pub async fn start(
        &self,
        namespace: DbNamespaceName,
        session: Option<&str>,
        table_name: &str,
        scope: BulkProcessScope,
        rows_by_partition: Vec<(String, Vec<Arc<DbRow>>)>,
        now: DateTimeAsMicroseconds,
    ) -> String {
        let mut write_access = self.items.lock().await;

        if let Some(session) = session {
            // Scoped by namespace as well: the same writer uploading the same
            // table name into two namespaces is two independent uploads, and
            // starting the second one must not throw away the first.
            let to_remove: Vec<String> = write_access
                .values()
                .filter(|itm| {
                    itm.session.as_deref() == Some(session)
                        && itm.table_name == table_name
                        && itm.namespace == namespace
                })
                .map(|itm| itm.id.clone())
                .collect();

            for id in to_remove {
                write_access.remove(&id);
            }
        }

        let id = generate_id(&write_access);

        let mut process = BulkProcess::new(
            id.clone(),
            namespace,
            session.map(|itm| itm.to_string()),
            table_name.to_string(),
            scope,
            now,
        );

        process.append(rows_by_partition, now);

        write_access.insert(id.clone(), process);

        id
    }

    pub async fn append(
        &self,
        id: &str,
        session: Option<&str>,
        table_name: &str,
        rows_by_partition: Vec<(String, Vec<Arc<DbRow>>)>,
        now: DateTimeAsMicroseconds,
    ) -> Result<(), BulkProcessError> {
        let mut write_access = self.items.lock().await;

        let process = get_mut(&mut write_access, id, session, Some(table_name))?;

        process.append(rows_by_partition, now);

        Ok(())
    }

    /// Takes the process out of the list. From this moment the caller owns the
    /// snapshot - a failed apply does not leave a half-committed process behind.
    pub async fn take(
        &self,
        id: &str,
        session: Option<&str>,
    ) -> Result<BulkProcess, BulkProcessError> {
        let mut write_access = self.items.lock().await;

        get_mut(&mut write_access, id, session, None)?;

        Ok(write_access.remove(id).unwrap())
    }

    pub async fn cancel(&self, id: &str, session: Option<&str>) -> Result<(), BulkProcessError> {
        self.take(id, session).await?;
        Ok(())
    }

    pub async fn gc(&self, now: DateTimeAsMicroseconds, timeout: Duration) {
        let mut write_access = self.items.lock().await;

        let to_remove: Vec<String> = write_access
            .values()
            .filter(|itm| now.duration_since(itm.last_update).as_positive_or_zero() >= timeout)
            .map(|itm| itm.id.clone())
            .collect();

        for id in to_remove {
            write_access.remove(&id);
        }
    }
}

fn get_mut<'s>(
    items: &'s mut BTreeMap<String, BulkProcess>,
    id: &str,
    session: Option<&str>,
    table_name: Option<&str>,
) -> Result<&'s mut BulkProcess, BulkProcessError> {
    let process = items.get_mut(id);

    if process.is_none() {
        return Err(BulkProcessError::ProcessNotFound { id: id.to_string() });
    }

    let process = process.unwrap();

    if process.session.as_deref() != session {
        return Err(BulkProcessError::ProcessDoesNotMatch {
            id: id.to_string(),
            reason: "Process belongs to another writer session".to_string(),
        });
    }

    if let Some(table_name) = table_name {
        if process.table_name != table_name {
            return Err(BulkProcessError::ProcessDoesNotMatch {
                id: id.to_string(),
                reason: format!("Process is started for table '{}'", process.table_name),
            });
        }
    }

    Ok(process)
}

fn generate_id(items: &BTreeMap<String, BulkProcess>) -> String {
    loop {
        let id = uuid::Uuid::new_v4().to_string();

        if !items.contains_key(&id) {
            return id;
        }
    }
}
