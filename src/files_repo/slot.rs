// Binary layout of one slot inside a size-class page-file. A slot is
// self-describing (carries its table + partition key), so recovery is a pure
// scan of the page-files — there is no separate key→location index on disk.
//
//   [0..4)    crc32      (u32 LE)  over bytes [4 .. 16 + body_len)
//   [4..12)   version    (u64 LE)  monotonic write counter (seeded from the
//                                  max on-disk version + 1 at scan); higher
//                                  wins on a duplicate after a crash
//                                  mid-relocation
//   [12..16)  body_len   (u32 LE)  length of body; 0 => slot is free
//   [16..)    body       table_len(u16) pk_len(u16) table pk zstd_payload
//   [...]     padding    zeroed up to the size class
//
// In-place overwrite of a multi-sector slot is not power-loss atomic; the crc
// detects a torn write on recovery so a broken slot is skipped, never loaded.

/// crc(4) + version(8) + body_len(4).
pub const SLOT_PREFIX_LEN: usize = 16;
/// table_len(2) + pk_len(2), at the start of the body.
const KEY_LEN_FIELDS: usize = 4;
/// Fixed per-slot overhead before the keys and the compressed payload.
pub const SLOT_OVERHEAD: usize = SLOT_PREFIX_LEN + KEY_LEN_FIELDS;

/// Total bytes a slot needs to hold this partition's compressed payload.
pub fn slot_bytes_needed(table_name: &str, partition_key: &str, payload_len: usize) -> usize {
    SLOT_OVERHEAD + table_name.len() + partition_key.len() + payload_len
}

/// Builds a full `size_class`-byte slot buffer (zero-padded) ready to be
/// written at the slot offset.
pub fn encode_slot(
    size_class: u32,
    version: u64,
    table_name: &str,
    partition_key: &str,
    payload: &[u8],
) -> Vec<u8> {
    let body_len = KEY_LEN_FIELDS + table_name.len() + partition_key.len() + payload.len();
    let mut buf = vec![0u8; size_class as usize];

    buf[4..12].copy_from_slice(&version.to_le_bytes());
    buf[12..16].copy_from_slice(&(body_len as u32).to_le_bytes());

    let mut pos = SLOT_PREFIX_LEN;
    buf[pos..pos + 2].copy_from_slice(&(table_name.len() as u16).to_le_bytes());
    pos += 2;
    buf[pos..pos + 2].copy_from_slice(&(partition_key.len() as u16).to_le_bytes());
    pos += 2;
    buf[pos..pos + table_name.len()].copy_from_slice(table_name.as_bytes());
    pos += table_name.len();
    buf[pos..pos + partition_key.len()].copy_from_slice(partition_key.as_bytes());
    pos += partition_key.len();
    buf[pos..pos + payload.len()].copy_from_slice(payload);
    pos += payload.len();

    let crc = crc32fast::hash(&buf[4..pos]);
    buf[0..4].copy_from_slice(&crc.to_le_bytes());

    buf
}

/// A successfully decoded, occupied slot.
pub struct OccupiedSlot {
    pub version: u64,
    pub table_name: String,
    pub partition_key: String,
    /// The compressed (zstd) partition payload.
    pub payload: Vec<u8>,
}

/// Result of decoding the bytes of one slot.
pub enum SlotState {
    /// `body_len == 0` — slot is empty / freed and available for reuse.
    Free,
    Occupied(OccupiedSlot),
    /// Length or crc check failed — a torn / corrupted write.
    Corrupt,
}

fn read_u32(src: &[u8]) -> u32 {
    u32::from_le_bytes(src.try_into().unwrap())
}

fn read_u16(src: &[u8]) -> u16 {
    u16::from_le_bytes(src.try_into().unwrap())
}

/// Decodes one slot from its full byte buffer (`bytes.len()` must equal the
/// size class).
pub fn decode_slot(bytes: &[u8]) -> SlotState {
    if bytes.len() < SLOT_PREFIX_LEN {
        return SlotState::Corrupt;
    }

    let crc_stored = read_u32(&bytes[0..4]);
    let version = u64::from_le_bytes(bytes[4..12].try_into().unwrap());
    let body_len = read_u32(&bytes[12..16]) as usize;

    if body_len == 0 {
        return SlotState::Free;
    }

    let end = SLOT_PREFIX_LEN + body_len;
    if end > bytes.len() {
        return SlotState::Corrupt;
    }

    if crc32fast::hash(&bytes[4..end]) != crc_stored {
        return SlotState::Corrupt;
    }

    let body = &bytes[SLOT_PREFIX_LEN..end];
    if body.len() < KEY_LEN_FIELDS {
        return SlotState::Corrupt;
    }

    let table_len = read_u16(&body[0..2]) as usize;
    let pk_len = read_u16(&body[2..4]) as usize;

    let mut pos = KEY_LEN_FIELDS;
    if pos + table_len + pk_len > body.len() {
        return SlotState::Corrupt;
    }

    let table_bytes = &body[pos..pos + table_len];
    pos += table_len;
    let pk_bytes = &body[pos..pos + pk_len];
    pos += pk_len;
    let payload = body[pos..].to_vec();

    let (Ok(table_name), Ok(partition_key)) = (
        String::from_utf8(table_bytes.to_vec()),
        String::from_utf8(pk_bytes.to_vec()),
    ) else {
        return SlotState::Corrupt;
    };

    SlotState::Occupied(OccupiedSlot {
        version,
        table_name,
        partition_key,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::super::size_class::size_class_for;
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let payload = vec![1u8, 2, 3, 250, 0, 7];
        let needed = slot_bytes_needed("my-table", "pk-42", payload.len());
        let size_class = size_class_for(needed);

        let buf = encode_slot(size_class, 12345, "my-table", "pk-42", &payload);
        assert_eq!(buf.len(), size_class as usize);

        match decode_slot(&buf) {
            SlotState::Occupied(slot) => {
                assert_eq!(slot.version, 12345);
                assert_eq!(slot.table_name, "my-table");
                assert_eq!(slot.partition_key, "pk-42");
                assert_eq!(slot.payload, payload);
            }
            _ => panic!("expected occupied slot"),
        }
    }

    #[test]
    fn zeroed_slot_is_free() {
        let buf = vec![0u8; 512];
        assert!(matches!(decode_slot(&buf), SlotState::Free));
    }

    #[test]
    fn torn_payload_is_corrupt() {
        let payload = vec![10u8; 100];
        let size_class = size_class_for(slot_bytes_needed("t", "p", payload.len()));
        let mut buf = encode_slot(size_class, 1, "t", "p", &payload);
        // Flip a byte inside the crc-covered body (a payload byte) without
        // fixing the crc -> must be detected as corrupt.
        buf[SLOT_PREFIX_LEN + 10] ^= 0xFF;
        assert!(matches!(decode_slot(&buf), SlotState::Corrupt));
    }

    #[test]
    fn empty_payload_roundtrips() {
        let size_class = size_class_for(slot_bytes_needed("t", "p", 0));
        let buf = encode_slot(size_class, 9, "t", "p", &[]);
        match decode_slot(&buf) {
            SlotState::Occupied(slot) => assert!(slot.payload.is_empty()),
            _ => panic!("expected occupied slot"),
        }
    }
}
