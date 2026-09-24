use std::collections::BTreeMap;

use ahash::AHashMap;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::persist_repo::{LoadedPartition, LoadedTableAttrs};
use crate::scripts::serializers::table_attrs::TableMetadataFileContract;

use super::size_class::{size_class_for, MIN_SIZE_CLASS};
use super::slot::{decode_slot, encode_slot, slot_bytes_needed, SlotState, SLOT_PREFIX_LEN};

const TABLES_META_FILE: &str = "tables.meta";

/// Where a partition's slot lives: which size-class page-file and which slot
/// index inside it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SlotLocation {
    pub size_class: u32,
    pub slot_index: u64,
}

impl SlotLocation {
    fn offset(&self) -> u64 {
        self.slot_index * self.size_class as u64
    }
}

/// Runtime state of one size-class page-file.
struct ClassState {
    /// Number of slots the file currently holds (file_len / size_class).
    slot_count: u64,
    /// Indices of freed slots available for reuse. In-memory only: a freed
    /// slot is self-describing on disk (`body_len == 0`), so the list is
    /// rebuilt by the recovery scan — nothing to persist.
    free: Vec<u64>,
}

/// The in-memory bookkeeping of the Files backend. Rebuilt entirely by scanning
/// the page-files on `load_all_partitions`, so nothing here needs to be a
/// crash-consistent on-disk structure besides the slots themselves.
pub struct FilesRepoInner {
    root: String,
    classes: AHashMap<u32, ClassState>,
    /// (table_name, partition_key) -> slot location.
    index: AHashMap<(String, String), SlotLocation>,
    /// table_name -> attributes (mirrored to `<root>/tables.meta`).
    tables: BTreeMap<String, TableMetadataFileContract>,
    /// Monotonic per-write counter used as the slot `version` for crash-time
    /// duplicate resolution. Seeded above the max version seen on disk during
    /// the recovery scan, so it stays monotonic across restarts and never
    /// depends on the (non-monotonic) wall clock.
    next_version: u64,
}

/// A slot picked up by the recovery scan.
struct ScannedSlot {
    key: (String, String),
    location: SlotLocation,
    version: u64,
    payload: Vec<u8>,
}

impl FilesRepoInner {
    pub async fn open(root: String, skip_errors: bool) -> Self {
        tokio::fs::create_dir_all(&root)
            .await
            .expect("files_repo: can not create root directory");

        let (tables, legacy_json) = load_tables_meta(&root, skip_errors).await;
        let classes = discover_class_files(&root).await;

        let result = Self {
            root,
            classes,
            index: AHashMap::new(),
            tables,
            next_version: 0,
        };

        // A tables.meta still in the legacy JSON format is converted to YAML
        // right away, so the migration does not wait for the next metadata
        // change (which may never come).
        if legacy_json && !result.tables.is_empty() {
            println!("files_repo: converting tables.meta from legacy json to yaml");
            result.persist_tables_meta().await;
        }

        result
    }

    fn class_path(&self, size_class: u32) -> String {
        format!("{}/{}", self.root, size_class)
    }

    // ---- reads / init ----------------------------------------------------

    pub fn get_tables(&self) -> Vec<LoadedTableAttrs> {
        self.tables
            .iter()
            .map(|(table_name, contract)| LoadedTableAttrs {
                table_name: table_name.clone().into(),
                attr: contract.clone().into(),
            })
            .collect()
    }

    /// Scans every page-file, rebuilds the in-memory index and free-lists, and
    /// returns every live partition's compressed payload. After a crash mid
    /// relocation two slots may carry the same key — the higher `version` wins
    /// and the loser is zeroed on disk so it can never resurrect later.
    pub async fn load_all_partitions(&mut self, skip_errors: bool) -> Vec<LoadedPartition> {
        let class_sizes: Vec<u32> = self.classes.keys().copied().collect();

        let mut scanned: Vec<ScannedSlot> = Vec::new();
        let mut max_version: u64 = 0;

        for size_class in class_sizes {
            let slot_count = self.classes.get(&size_class).unwrap().slot_count;
            let path = self.class_path(size_class);
            // An unreadable page-file is always fatal, even with
            // SkipBrokenPartitions: unlike a corrupt slot (which the scan zeroes
            // so it can never resurrect), an unread file can not be neutralized —
            // its partitions would silently vanish for this run and its versions
            // would be missing from the `next_version` seed, so partitions
            // re-saved later would lose the dedup against the stale slots on the
            // following restart and be destroyed. NotFound is tolerated: the file
            // disappeared after discovery, so the class is simply empty.
            let file_bytes = match tokio::fs::read(&path).await {
                Ok(bytes) => bytes,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(err) => panic!("files_repo: can not read page-file {}: {}", path, err),
            };
            let slot_len = size_class as usize;

            for slot_index in 0..slot_count {
                let start = slot_index as usize * slot_len;
                let end = start + slot_len;
                if end > file_bytes.len() {
                    break;
                }

                match decode_slot(&file_bytes[start..end]) {
                    SlotState::Free => {
                        self.classes
                            .get_mut(&size_class)
                            .unwrap()
                            .free
                            .push(slot_index);
                    }
                    SlotState::Corrupt => {
                        if !skip_errors {
                            panic!(
                                "files_repo: corrupt slot {} in page-file {}",
                                slot_index, path
                            );
                        }
                        println!(
                            "files_repo: skipping corrupt slot {} in {}",
                            slot_index, path
                        );
                        // Reuse the broken slot; it can never be decoded to a key.
                        self.classes
                            .get_mut(&size_class)
                            .unwrap()
                            .free
                            .push(slot_index);
                    }
                    SlotState::Occupied(slot) => {
                        if slot.version > max_version {
                            max_version = slot.version;
                        }
                        scanned.push(ScannedSlot {
                            key: (slot.table_name, slot.partition_key),
                            location: SlotLocation {
                                size_class,
                                slot_index,
                            },
                            version: slot.version,
                            payload: slot.payload,
                        });
                    }
                }
            }
        }

        // Deduplicate by key keeping the highest version; losers are freed.
        let mut winners: AHashMap<(String, String), ScannedSlot> = AHashMap::new();
        let mut losers: Vec<SlotLocation> = Vec::new();

        for slot in scanned {
            match winners.entry(slot.key.clone()) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    if slot.version > e.get().version {
                        let old = e.insert(slot);
                        losers.push(old.location);
                    } else {
                        losers.push(slot.location);
                    }
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(slot);
                }
            }
        }

        for location in losers {
            self.zero_slot(location).await;
            self.classes
                .get_mut(&location.size_class)
                .unwrap()
                .free
                .push(location.slot_index);
        }

        // Seed the write counter above every version seen on disk so it stays
        // monotonic across restarts (losers were just freed; winners are kept).
        self.next_version = max_version + 1;

        let mut result = Vec::with_capacity(winners.len());
        for (key, slot) in winners {
            self.index.insert(key.clone(), slot.location);
            result.push(LoadedPartition {
                table_name: key.0,
                partition_key: key.1,
                compressed: slot.payload,
            });
        }

        result
    }

    // ---- writes ----------------------------------------------------------

    pub async fn save_partition(&mut self, table_name: &str, partition_key: &str, payload: &[u8]) {
        let needed = slot_bytes_needed(table_name, partition_key, payload.len());
        let size_class = size_class_for(needed);
        let key = (table_name.to_string(), partition_key.to_string());
        let version = self.next_version;
        self.next_version += 1;

        let existing = self.index.get(&key).copied();

        // Synchronous bookkeeping: choose the target slot (in place when the
        // size class is unchanged, otherwise allocate / reuse a free slot).
        let target = match existing {
            Some(location) if location.size_class == size_class => location,
            _ => self.allocate_slot(size_class),
        };

        self.index.insert(key, target);

        // Disk writes (no fsync — durability decision).
        let buf = encode_slot(size_class, version, table_name, partition_key, payload);
        self.write_slot(target, &buf).await;

        if let Some(old) = existing {
            if old != target {
                self.zero_slot(old).await;
                self.classes
                    .get_mut(&old.size_class)
                    .unwrap()
                    .free
                    .push(old.slot_index);
            }
        }
    }

    pub async fn delete_partition(&mut self, table_name: &str, partition_key: &str) {
        let key = (table_name.to_string(), partition_key.to_string());
        if let Some(location) = self.index.remove(&key) {
            self.zero_slot(location).await;
            self.classes
                .get_mut(&location.size_class)
                .unwrap()
                .free
                .push(location.slot_index);
        }
    }

    pub async fn clean_table_content(&mut self, table_name: &str) {
        let to_free: Vec<((String, String), SlotLocation)> = self
            .index
            .iter()
            .filter(|((t, _), _)| t == table_name)
            .map(|(k, loc)| (k.clone(), *loc))
            .collect();

        for (key, location) in to_free {
            self.index.remove(&key);
            self.zero_slot(location).await;
            self.classes
                .get_mut(&location.size_class)
                .unwrap()
                .free
                .push(location.slot_index);
        }
    }

    /// Replaces the whole table's content: writes/overwrites every supplied
    /// partition first, then frees only the slots of partitions that are no
    /// longer present. Unlike clean-then-write, this never frees a partition's
    /// slot before its replacement is on disk.
    pub async fn replace_table_partitions(
        &mut self,
        table_name: &str,
        partitions: Vec<(String, Vec<u8>)>,
    ) {
        let new_keys: std::collections::HashSet<&str> =
            partitions.iter().map(|(pk, _)| pk.as_str()).collect();

        for (partition_key, payload) in &partitions {
            self.save_partition(table_name, partition_key, payload)
                .await;
        }

        let stale: Vec<(String, String)> = self
            .index
            .keys()
            .filter(|(t, pk)| t == table_name && !new_keys.contains(pk.as_str()))
            .cloned()
            .collect();

        for key in stale {
            if let Some(location) = self.index.remove(&key) {
                self.zero_slot(location).await;
                self.classes
                    .get_mut(&location.size_class)
                    .unwrap()
                    .free
                    .push(location.slot_index);
            }
        }
    }

    pub async fn save_table_metadata(
        &mut self,
        table_name: &str,
        contract: TableMetadataFileContract,
    ) {
        self.tables.insert(table_name.to_string(), contract);
        self.persist_tables_meta().await;
    }

    pub async fn delete_table_metadata(&mut self, table_name: &str) {
        if self.tables.remove(table_name).is_some() {
            self.persist_tables_meta().await;
        }
    }

    /// Reclaims disk in two phases. (1) Any page-file whose every slot is free
    /// (all slot indices present in the free-list) is fully dead, so the
    /// `<size>` file is removed and the class dropped from memory; a future
    /// write to that size class recreates the file from scratch. (2) A file
    /// with more than two slots where at least half are free is compacted:
    /// live slots from the tail move into the free slots at the head and the
    /// file is truncated to exactly its live slots (at least a 2x shrink given
    /// the trigger). Other files are left untouched — their free slots are
    /// reused in place. Runs under the repo mutex, so it never races a write.
    pub async fn vacuum(&mut self) {
        let empty_classes: Vec<u32> = self
            .classes
            .iter()
            .filter(|(_, state)| {
                if state.slot_count == 0 {
                    return false;
                }
                // Every slot index in the file must be in the free-list.
                let free: std::collections::HashSet<u64> = state.free.iter().copied().collect();
                (0..state.slot_count).all(|slot_index| free.contains(&slot_index))
            })
            .map(|(size_class, _)| *size_class)
            .collect();

        for size_class in empty_classes {
            // Treat "already gone" as success.
            let removed = match tokio::fs::remove_file(self.class_path(size_class)).await {
                Ok(_) => true,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
                Err(err) => {
                    println!(
                        "files_repo: could not vacuum page-file {}: {} (will retry)",
                        size_class, err
                    );
                    false
                }
            };

            if removed {
                self.classes.remove(&size_class);
                println!(
                    "files_repo: vacuumed fully-freed page-file for size class {}",
                    size_class
                );
            }
        }

        let compact_candidates: Vec<u32> = self
            .classes
            .iter()
            .filter(|(_, state)| {
                // Fully-free classes are phase 1's job: if their file removal
                // failed above ("will retry"), compacting them here would just
                // re-hit the same broken file.
                state.slot_count > 2
                    && (state.free.len() as u64) < state.slot_count
                    && state.free.len() as u64 * 2 >= state.slot_count
            })
            .map(|(size_class, _)| *size_class)
            .collect();

        for size_class in compact_candidates {
            self.compact_class(size_class).await;
        }
    }

    /// Compacts a half-empty page-file: every live slot sitting at an index
    /// beyond the new end of file is copied verbatim (crc and version
    /// included, so the copy is valid by construction) into a free slot at the
    /// head, the in-memory index is repointed, and the file is truncated to
    /// exactly its live slots.
    ///
    /// Crash safety, in copy-fsync-truncate order: a torn copy fails its crc
    /// on the next scan and is skipped while the original tail slot still
    /// holds the data; a completed copy that crashes before the truncate
    /// leaves two valid slots with the SAME version — the head copy is
    /// scanned first and wins the dedup, the tail original is zeroed as the
    /// loser. The copies are fsynced BEFORE the truncate so the filesystem
    /// can never make the truncate durable ahead of the copied data (that
    /// order would destroy both the originals and the copies on power loss).
    async fn compact_class(&mut self, size_class: u32) {
        let state = self.classes.get(&size_class).unwrap();
        let slot_count = state.slot_count;
        let live_count = slot_count - state.free.len() as u64;

        // Copy targets: free slots below the new end of file.
        let head_frees: Vec<u64> = state
            .free
            .iter()
            .copied()
            .filter(|slot_index| *slot_index < live_count)
            .collect();

        // Slots to move: live (indexed) slots at or beyond the new end of file.
        let tail_lives: Vec<((String, String), u64)> = self
            .index
            .iter()
            .filter(|(_, location)| {
                location.size_class == size_class && location.slot_index >= live_count
            })
            .map(|(key, location)| (key.clone(), location.slot_index))
            .collect();

        // Live and free slots partition 0..slot_count, so the tail lives and
        // the head frees always pair up one to one.
        assert_eq!(
            tail_lives.len(),
            head_frees.len(),
            "files_repo: free-list / index mismatch in size class {}",
            size_class
        );

        let path = self.class_path(size_class);
        let mut file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .await
            .expect("files_repo: can not open page-file for compaction");

        let mut buf = vec![0u8; size_class as usize];
        for ((_, from_index), to_index) in tail_lives.iter().zip(head_frees.iter()) {
            file.seek(std::io::SeekFrom::Start(from_index * size_class as u64))
                .await
                .expect("files_repo: seek failed");
            file.read_exact(&mut buf)
                .await
                .expect("files_repo: compaction read failed");
            file.seek(std::io::SeekFrom::Start(to_index * size_class as u64))
                .await
                .expect("files_repo: seek failed");
            file.write_all(&buf)
                .await
                .expect("files_repo: compaction write failed");
        }
        file.flush()
            .await
            .expect("files_repo: compaction flush failed");
        // fsync BEFORE the truncate — the one place in this backend that needs
        // it: a truncate is a metadata operation the filesystem may journal as
        // durable before the copied slot DATA reaches disk. On power loss that
        // order would destroy the tail originals (truncated) while the head
        // copies never made it — losing the moved partitions entirely. Syncing
        // the copies first closes that window; compaction is an hourly, rare
        // event, so the cost is irrelevant.
        file.sync_all()
            .await
            .expect("files_repo: compaction fsync failed");
        file.set_len(live_count * size_class as u64)
            .await
            .expect("files_repo: compaction truncate failed");

        for ((key, _), to_index) in tail_lives.into_iter().zip(head_frees.into_iter()) {
            self.index.insert(
                key,
                SlotLocation {
                    size_class,
                    slot_index: to_index,
                },
            );
        }

        let state = self.classes.get_mut(&size_class).unwrap();
        state.slot_count = live_count;
        state.free.clear();

        println!(
            "files_repo: compacted page-file {}: {} -> {} slots",
            size_class, slot_count, live_count
        );
    }

    // ---- low-level helpers ----------------------------------------------

    /// Reserves a slot index in `size_class`, reusing a freed one when
    /// available, otherwise extending the file by one slot.
    fn allocate_slot(&mut self, size_class: u32) -> SlotLocation {
        let state = self.classes.entry(size_class).or_insert(ClassState {
            slot_count: 0,
            free: Vec::new(),
        });

        if let Some(slot_index) = state.free.pop() {
            return SlotLocation {
                size_class,
                slot_index,
            };
        }

        let slot_index = state.slot_count;
        state.slot_count += 1;
        SlotLocation {
            size_class,
            slot_index,
        }
    }

    async fn write_slot(&self, location: SlotLocation, buf: &[u8]) {
        let path = self.class_path(location.size_class);
        let mut file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            // Never truncate: we overwrite one slot in place and keep the rest
            // of the page-file intact.
            .truncate(false)
            .open(&path)
            .await
            .expect("files_repo: can not open page-file for write");
        file.seek(std::io::SeekFrom::Start(location.offset()))
            .await
            .expect("files_repo: seek failed");
        file.write_all(buf)
            .await
            .expect("files_repo: slot write failed");
        // tokio buffers the write and hands it to the blocking pool; flush waits
        // for that write to land in the OS (it does NOT fsync), so reads through
        // other fds in this process observe it. Power-loss durability stays
        // best-effort by design.
        file.flush().await.expect("files_repo: slot flush failed");
    }

    /// Marks a slot free by zeroing its 16-byte prefix (`body_len = 0`), so the
    /// recovery scan treats it as reusable and never reloads its old content.
    async fn zero_slot(&self, location: SlotLocation) {
        let path = self.class_path(location.size_class);
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .await
            .expect("files_repo: can not open page-file to free a slot");
        file.seek(std::io::SeekFrom::Start(location.offset()))
            .await
            .expect("files_repo: seek failed");
        file.write_all(&[0u8; SLOT_PREFIX_LEN])
            .await
            .expect("files_repo: zeroing slot failed");
        // Same as write_slot: wait for the buffered write, no fsync.
        file.flush().await.expect("files_repo: slot flush failed");
    }

    async fn persist_tables_meta(&self) {
        let yaml = serde_yaml::to_string(&self.tables).unwrap();
        atomic_write(
            &format!("{}/{}", self.root, TABLES_META_FILE),
            yaml.as_bytes(),
        )
        .await;
    }
}

/// Reads `tables.meta`, returning the map and whether the file was found in
/// the legacy JSON format (the caller then rewrites it as YAML right away).
/// A missing file is a fresh directory (empty map). The file is YAML; files
/// written before the format switch were JSON, so a failed YAML parse falls
/// back to JSON. Because JSON is itself valid YAML, the parse branch alone can
/// not tell the formats apart — the raw bytes are probed with a JSON parse
/// instead (the YAML writer emits block style, which never parses as JSON).
/// A read error or a file neither format can parse is fatal unless
/// `skip_errors` (SkipBrokenPartitions) is set — silently defaulting would
/// reset every table's attributes and let the next metadata write overwrite
/// the still-recoverable file.
async fn load_tables_meta(
    root: &str,
    skip_errors: bool,
) -> (BTreeMap<String, TableMetadataFileContract>, bool) {
    let path = format!("{}/{}", root, TABLES_META_FILE);
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return (BTreeMap::new(), false),
        Err(err) => panic!("files_repo: can not read {}: {}", path, err),
    };

    let is_legacy_json = serde_json::from_slice::<serde_json::Value>(&bytes).is_ok();

    let yaml_err = match serde_yaml::from_slice(&bytes) {
        Ok(tables) => return (tables, is_legacy_json),
        Err(err) => err,
    };

    match serde_json::from_slice(&bytes) {
        Ok(tables) => (tables, true),
        Err(json_err) => {
            let msg = format!(
                "files_repo: can not parse {} as yaml ({}) nor as legacy json ({})",
                path, yaml_err, json_err
            );
            if skip_errors {
                println!("{}. Table attributes will be restored with defaults.", msg);
                (BTreeMap::new(), false)
            } else {
                panic!("{}", msg);
            }
        }
    }
}

/// Lists the persistence root and records every page-file's slot count. Any
/// listing / stat error is fatal — silently dropping a size class here would
/// bypass the recovery scan exactly like an unreadable page-file: its
/// partitions vanish for the run and `next_version` gets seeded below the
/// unscanned slots' versions, so data re-saved later would lose the dedup and
/// be destroyed on the following restart.
async fn discover_class_files(root: &str) -> AHashMap<u32, ClassState> {
    let mut classes = AHashMap::new();

    let mut read_dir = tokio::fs::read_dir(root).await.unwrap_or_else(|err| {
        panic!(
            "files_repo: can not list persistence root {}: {}",
            root, err
        )
    });

    loop {
        let entry = read_dir.next_entry().await.unwrap_or_else(|err| {
            panic!(
                "files_repo: can not list persistence root {}: {}",
                root, err
            )
        });
        let Some(entry) = entry else {
            break;
        };

        let file_type = entry
            .file_type()
            .await
            .unwrap_or_else(|err| panic!("files_repo: can not stat {:?}: {}", entry.path(), err));
        if !file_type.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };

        // Page-files are named by their size class (a plain integer).
        let Ok(size_class) = file_name.parse::<u32>() else {
            continue;
        };
        if size_class < MIN_SIZE_CLASS {
            continue;
        }

        let len = entry
            .metadata()
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "files_repo: can not stat page-file {:?}: {}",
                    entry.path(),
                    err
                )
            })
            .len();
        let slot_count = len / size_class as u64;

        classes.insert(
            size_class,
            ClassState {
                slot_count,
                free: Vec::new(),
            },
        );
    }

    classes
}

/// Writes `bytes` to `path` atomically: tmp file -> fsync -> rename. Used for
/// `tables.meta`, where a torn write would be costly; the slots themselves are
/// deliberately written without fsync.
async fn atomic_write(path: &str, bytes: &[u8]) {
    let tmp_path = format!("{}.tmp", path);
    {
        let mut file = tokio::fs::File::create(&tmp_path)
            .await
            .expect("files_repo: can not create tmp file");
        file.write_all(bytes)
            .await
            .expect("files_repo: tmp write failed");
        file.sync_all().await.expect("files_repo: tmp fsync failed");
    }
    tokio::fs::rename(&tmp_path, path)
        .await
        .expect("files_repo: rename failed");
}
