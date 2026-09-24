//! End-to-end tests of the FilesRepo backend: they exercise the public
//! wrapper against a real temp directory, including reopen (recovery scan),
//! slot reuse, relocation between size classes, vacuum and corruption skip.

use my_no_sql_sdk::core::db::DbTableAttributes;

use crate::persist_repo::LoadedPartition;

use super::FilesRepo;

/// Creates a unique empty directory for one test and returns its path.
fn new_test_dir() -> String {
    let dir = std::env::temp_dir().join(format!("files_repo_it_{}", uuid::Uuid::new_v4()));
    dir.to_str().unwrap().to_string()
}

async fn cleanup(dir: &str) {
    tokio::fs::remove_dir_all(dir).await.ok();
}

async fn file_len(path: &str) -> Option<u64> {
    match tokio::fs::metadata(path).await {
        Ok(meta) => Some(meta.len()),
        Err(_) => None,
    }
}

/// Opens the repo on `dir` and performs the init-time recovery scan, exactly
/// like the production init flow does before any writes.
async fn reopen(dir: &str, skip_errors: bool) -> (FilesRepo, Vec<LoadedPartition>) {
    let repo = FilesRepo::open(dir.to_string(), skip_errors).await;
    let loaded = repo.load_all_partitions(skip_errors).await;
    (repo, loaded)
}

fn find_payload<'s>(loaded: &'s [LoadedPartition], table: &str, pk: &str) -> Option<&'s [u8]> {
    loaded
        .iter()
        .find(|p| p.table_name == table && p.partition_key == pk)
        .map(|p| p.compressed.as_slice())
}

fn payload(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

#[tokio::test]
async fn round_trip_across_size_classes_and_table_metadata() {
    let dir = new_test_dir();
    let (repo, loaded) = reopen(&dir, false).await;
    assert!(loaded.is_empty());

    // Payload sizes chosen to land in three different size classes.
    let small = payload(100, 1); // -> 512 class
    let medium = payload(700, 2); // -> 1024 class
    let large = payload(3000, 3); // -> 4096 class
    let small2 = payload(40, 4); // -> 512 class, second slot

    repo.save_partition("tbl-a", "pk-small", &small).await;
    repo.save_partition("tbl-a", "pk-medium", &medium).await;
    repo.save_partition("tbl-b", "pk-large", &large).await;
    repo.save_partition("tbl-b", "pk-small2", &small2).await;

    let attrs = DbTableAttributes {
        persist: true,
        max_partitions_amount: Some(7),
        max_rows_per_partition_amount: None,
        compressed: true,
        created: my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds::now(),
    };
    repo.save_table_metadata("tbl-a", &attrs).await;
    drop(repo);

    let (repo, loaded) = reopen(&dir, false).await;
    assert_eq!(loaded.len(), 4);
    assert_eq!(
        find_payload(&loaded, "tbl-a", "pk-small"),
        Some(small.as_slice())
    );
    assert_eq!(
        find_payload(&loaded, "tbl-a", "pk-medium"),
        Some(medium.as_slice())
    );
    assert_eq!(
        find_payload(&loaded, "tbl-b", "pk-large"),
        Some(large.as_slice())
    );
    assert_eq!(
        find_payload(&loaded, "tbl-b", "pk-small2"),
        Some(small2.as_slice())
    );

    let tables = repo.get_tables().await;
    assert_eq!(tables.len(), 1);
    let table = &tables[0];
    assert_eq!(table.table_name.as_str(), "tbl-a");
    assert!(table.attr.persist);
    assert_eq!(table.attr.max_partitions_amount, Some(7));
    assert_eq!(table.attr.max_rows_per_partition_amount, None);
    assert!(table.attr.compressed);

    cleanup(&dir).await;
}

#[tokio::test]
async fn legacy_json_tables_meta_loads_and_is_rewritten_as_yaml() {
    let dir = new_test_dir();
    tokio::fs::create_dir_all(&dir).await.unwrap();

    // A tables.meta written by the pre-YAML format (JSON).
    let legacy = r#"{"legacy-table":{"Persist":true,"MaxPartitionsAmount":7,"MaxRowsPerPartitionAmount":null,"Compressed":true,"Created":null}}"#;
    tokio::fs::write(format!("{}/tables.meta", dir), legacy)
        .await
        .unwrap();

    let (repo, _) = reopen(&dir, false).await;
    let tables = repo.get_tables().await;
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].table_name.as_str(), "legacy-table");
    assert!(tables[0].attr.persist);
    assert_eq!(tables[0].attr.max_partitions_amount, Some(7));
    assert!(tables[0].attr.compressed);
    drop(repo);

    // The legacy file is converted to YAML right at open, before any
    // metadata change happens.
    let bytes = tokio::fs::read(format!("{}/tables.meta", dir))
        .await
        .unwrap();
    assert!(
        serde_json::from_slice::<serde_json::Value>(&bytes).is_err(),
        "tables.meta is expected to be converted to YAML at open, got JSON"
    );

    // And the converted file still round-trips with the attributes intact.
    let (repo, _) = reopen(&dir, false).await;
    let tables = repo.get_tables().await;
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].attr.max_partitions_amount, Some(7));
    drop(repo);

    cleanup(&dir).await;
}

#[tokio::test]
async fn in_place_overwrite_keeps_one_slot_with_latest_payload() {
    let dir = new_test_dir();
    let (repo, _) = reopen(&dir, false).await;

    let first = payload(120, 10);
    let second = payload(150, 20); // same 512 size class

    repo.save_partition("tbl", "pk", &first).await;
    repo.save_partition("tbl", "pk", &second).await;
    drop(repo);

    // Exactly one slot: the page-file must not have grown by a second slot.
    let page_file = format!("{}/512", dir);
    assert_eq!(file_len(&page_file).await, Some(512));

    let (_repo, loaded) = reopen(&dir, false).await;
    assert_eq!(loaded.len(), 1);
    assert_eq!(find_payload(&loaded, "tbl", "pk"), Some(second.as_slice()));

    cleanup(&dir).await;
}

#[tokio::test]
async fn relocation_to_bigger_class_frees_old_slot_for_reuse() {
    let dir = new_test_dir();
    let (repo, _) = reopen(&dir, false).await;

    let small = payload(100, 5); // -> 512 class
    let large = payload(700, 6); // -> 1024 class, forces relocation

    repo.save_partition("tbl", "pk", &small).await;
    repo.save_partition("tbl", "pk", &large).await;
    drop(repo);

    let old_class_file = format!("{}/512", dir);
    let new_class_file = format!("{}/1024", dir);
    assert_eq!(file_len(&old_class_file).await, Some(512));
    assert_eq!(file_len(&new_class_file).await, Some(1024));

    let (repo, loaded) = reopen(&dir, false).await;
    // Exactly one copy survives: the latest (relocated) payload.
    assert_eq!(loaded.len(), 1);
    assert_eq!(find_payload(&loaded, "tbl", "pk"), Some(large.as_slice()));

    // The freed slot in the old class is reused: saving another small
    // partition must not grow the 512 page-file.
    let another_small = payload(90, 7);
    repo.save_partition("tbl", "pk2", &another_small).await;
    assert_eq!(file_len(&old_class_file).await, Some(512));
    drop(repo);

    let (_repo, loaded) = reopen(&dir, false).await;
    assert_eq!(loaded.len(), 2);
    assert_eq!(find_payload(&loaded, "tbl", "pk"), Some(large.as_slice()));
    assert_eq!(
        find_payload(&loaded, "tbl", "pk2"),
        Some(another_small.as_slice())
    );

    cleanup(&dir).await;
}

#[tokio::test]
async fn vacuum_removes_fully_freed_page_file_and_class_is_recreated() {
    let dir = new_test_dir();
    let (repo, _) = reopen(&dir, false).await;

    let data = payload(100, 8);
    repo.save_partition("tbl", "pk", &data).await;
    repo.delete_partition("tbl", "pk").await;

    let page_file = format!("{}/512", dir);
    assert!(file_len(&page_file).await.is_some());

    repo.vacuum().await;
    assert_eq!(
        file_len(&page_file).await,
        None,
        "page-file must be removed"
    );

    // A new save into the same class recreates the file from scratch.
    let fresh = payload(80, 9);
    repo.save_partition("tbl", "pk-new", &fresh).await;
    assert_eq!(file_len(&page_file).await, Some(512));
    drop(repo);

    let (_repo, loaded) = reopen(&dir, false).await;
    assert_eq!(loaded.len(), 1);
    assert_eq!(
        find_payload(&loaded, "tbl", "pk-new"),
        Some(fresh.as_slice())
    );

    cleanup(&dir).await;
}

#[tokio::test]
async fn vacuum_keeps_page_file_with_a_live_slot() {
    let dir = new_test_dir();
    let (repo, _) = reopen(&dir, false).await;

    let live = payload(100, 11);
    let dead = payload(110, 12);
    repo.save_partition("tbl", "pk-live", &live).await;
    repo.save_partition("tbl", "pk-dead", &dead).await;
    repo.delete_partition("tbl", "pk-dead").await;

    repo.vacuum().await;

    let page_file = format!("{}/512", dir);
    assert_eq!(
        file_len(&page_file).await,
        Some(1024),
        "partially-free page-file must survive vacuum"
    );
    drop(repo);

    let (_repo, loaded) = reopen(&dir, false).await;
    assert_eq!(loaded.len(), 1);
    assert_eq!(
        find_payload(&loaded, "tbl", "pk-live"),
        Some(live.as_slice())
    );

    cleanup(&dir).await;
}

#[tokio::test]
async fn vacuum_compacts_half_freed_page_file() {
    let dir = new_test_dir();
    let (repo, _) = reopen(&dir, false).await;

    // Four slots in the 512 class; the two HEAD slots get freed, so both
    // live partitions sit in the tail and must be moved by the compaction.
    let p0 = payload(100, 20);
    let p1 = payload(100, 21);
    let p2 = payload(100, 22);
    let p3 = payload(100, 23);
    repo.save_partition("tbl", "pk-0", &p0).await;
    repo.save_partition("tbl", "pk-1", &p1).await;
    repo.save_partition("tbl", "pk-2", &p2).await;
    repo.save_partition("tbl", "pk-3", &p3).await;
    repo.delete_partition("tbl", "pk-0").await;
    repo.delete_partition("tbl", "pk-1").await;

    let page_file = format!("{}/512", dir);
    assert_eq!(file_len(&page_file).await, Some(4 * 512));

    repo.vacuum().await;
    assert_eq!(
        file_len(&page_file).await,
        Some(2 * 512),
        "half-freed page-file must shrink to its live slots"
    );

    // Post-compaction bookkeeping: the free-list is empty, so the next save
    // appends a third slot.
    let p4 = payload(90, 24);
    repo.save_partition("tbl", "pk-4", &p4).await;
    assert_eq!(file_len(&page_file).await, Some(3 * 512));
    drop(repo);

    let (_repo, loaded) = reopen(&dir, false).await;
    assert_eq!(loaded.len(), 3);
    assert_eq!(find_payload(&loaded, "tbl", "pk-2"), Some(p2.as_slice()));
    assert_eq!(find_payload(&loaded, "tbl", "pk-3"), Some(p3.as_slice()));
    assert_eq!(find_payload(&loaded, "tbl", "pk-4"), Some(p4.as_slice()));
    assert_eq!(find_payload(&loaded, "tbl", "pk-0"), None);
    assert_eq!(find_payload(&loaded, "tbl", "pk-1"), None);

    cleanup(&dir).await;
}

#[tokio::test]
async fn vacuum_does_not_compact_two_slot_file() {
    let dir = new_test_dir();
    let (repo, _) = reopen(&dir, false).await;

    let live = payload(100, 25);
    let dead = payload(100, 26);
    repo.save_partition("tbl", "pk-live", &live).await;
    repo.save_partition("tbl", "pk-dead", &dead).await;
    repo.delete_partition("tbl", "pk-dead").await;

    repo.vacuum().await;

    // Compaction requires more than two slots; the file keeps its size and
    // the free slot is reused in place instead.
    let page_file = format!("{}/512", dir);
    assert_eq!(file_len(&page_file).await, Some(2 * 512));
    drop(repo);

    let (_repo, loaded) = reopen(&dir, false).await;
    assert_eq!(loaded.len(), 1);
    assert_eq!(
        find_payload(&loaded, "tbl", "pk-live"),
        Some(live.as_slice())
    );

    cleanup(&dir).await;
}

#[tokio::test]
async fn corrupt_slot_is_skipped_and_the_rest_loads() {
    let dir = new_test_dir();
    let (repo, _) = reopen(&dir, false).await;

    let first = payload(200, 13); // slot 0 of the 512 class
    let second = payload(210, 14); // slot 1
    repo.save_partition("tbl", "pk-first", &first).await;
    repo.save_partition("tbl", "pk-second", &second).await;
    drop(repo);

    // Flip one byte inside the FIRST slot's payload region (crc-covered):
    // prefix(16) + key-len fields(4) + table + pk, then 10 bytes into payload.
    let page_file = format!("{}/512", dir);
    let mut bytes = tokio::fs::read(&page_file).await.unwrap();
    let payload_start = 16 + 4 + "tbl".len() + "pk-first".len();
    bytes[payload_start + 10] ^= 0xFF;
    tokio::fs::write(&page_file, &bytes).await.unwrap();

    let (_repo, loaded) = reopen(&dir, true).await;
    assert_eq!(loaded.len(), 1, "corrupt slot must be skipped");
    assert_eq!(
        find_payload(&loaded, "tbl", "pk-second"),
        Some(second.as_slice())
    );
    assert_eq!(find_payload(&loaded, "tbl", "pk-first"), None);

    cleanup(&dir).await;
}
