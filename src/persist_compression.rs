// Shared zstd codec for persisted partition payloads. Compression is CPU-heavy,
// so callers must invoke it OUTSIDE any DB / partition lock (see Performance
// Considerations §6).

/// zstd level used for persisted partitions. Matches the level used by the
/// HTTP layer (`http_server::mappers::compression`).
const ZSTD_LEVEL: i32 = 11;

/// Compresses a partition payload (a JSON array of rows). Panics only if zstd
/// itself fails, which for in-memory input means OOM — nothing to recover from.
pub fn compress(raw: &[u8]) -> Vec<u8> {
    zstd::encode_all(raw, ZSTD_LEVEL).unwrap()
}

/// Decompresses a previously `compress`-ed payload. Returns `Err` on corrupt
/// input so the caller (init / recovery scan) can skip a broken partition
/// instead of crashing.
pub fn decompress(compressed: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    zstd::decode_all(compressed)
}
