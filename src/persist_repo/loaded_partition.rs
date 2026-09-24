/// One persisted partition as read back by a backend on init: the compressed
/// (zstd) JSON array of the partition's rows. The init path decompresses and
/// parses it via `DbJsonEntity::restore_as_vec`.
pub struct LoadedPartition {
    pub table_name: String,
    pub partition_key: String,
    pub compressed: Vec<u8>,
}
