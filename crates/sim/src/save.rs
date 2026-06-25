//! Save codec: postcard → deflate → frame(magic + version + crc32) → base64url.
//! Pure and headless-testable. Powers both localStorage and shareable links.

use crate::state::{GameState, SAVE_VERSION};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;

const MAGIC: &[u8; 4] = b"CSV1";

/// Hard cap on inflated save size — stops a crafted "zip bomb" link from
/// exhausting memory. Generous vs real saves (tens of bytes at M0), tiny vs a DoS.
const MAX_DECOMPRESSED_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    Serialize,
    Deserialize,
    Decompress,
    Base64,
    Checksum,
    BadMagic,
    Version(u32),
}

pub fn encode(state: &GameState) -> Result<String, SaveError> {
    let bytes = postcard::to_allocvec(state).map_err(|_| SaveError::Serialize)?;
    let compressed = miniz_oxide::deflate::compress_to_vec(&bytes, 6);
    let crc = crc32fast::hash(&compressed);
    let mut framed = Vec::with_capacity(12 + compressed.len());
    framed.extend_from_slice(MAGIC);
    framed.extend_from_slice(&SAVE_VERSION.to_le_bytes()); // schema version OUTSIDE the payload
    framed.extend_from_slice(&crc.to_le_bytes());
    framed.extend_from_slice(&compressed);
    Ok(B64.encode(framed))
}

/// Reverse of `encode`, validating magic, checksum, and version.
pub fn decode(s: &str) -> Result<GameState, SaveError> {
    decode_with_limit(s, MAX_DECOMPRESSED_BYTES)
}

fn decode_with_limit(s: &str, max_decompressed: usize) -> Result<GameState, SaveError> {
    let framed = B64.decode(s.trim()).map_err(|_| SaveError::Base64)?;
    if framed.len() < 12 || &framed[0..4] != MAGIC {
        return Err(SaveError::BadMagic);
    }
    let version = u32::from_le_bytes([framed[4], framed[5], framed[6], framed[7]]);
    let crc = u32::from_le_bytes([framed[8], framed[9], framed[10], framed[11]]);
    let compressed = &framed[12..];
    if crc32fast::hash(compressed) != crc {
        return Err(SaveError::Checksum);
    }
    // Version is read from the frame BEFORE touching the payload, so a future
    // schema can be migrated (or cleanly rejected) without first deserializing
    // into the current GameState shape.
    migrate(version, compressed, max_decompressed)
}

/// Migration seam: dispatch on the on-disk schema version. Future versions add
/// arms that read their own layout and upgrade to the current `GameState`.
fn migrate(
    version: u32,
    compressed: &[u8],
    max_decompressed: usize,
) -> Result<GameState, SaveError> {
    match version {
        SAVE_VERSION => {
            let bytes =
                miniz_oxide::inflate::decompress_to_vec_with_limit(compressed, max_decompressed)
                    .map_err(|_| SaveError::Decompress)?;
            postcard::from_bytes(&bytes).map_err(|_| SaveError::Deserialize)
        }
        other => Err(SaveError::Version(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Tuning;

    fn sample() -> GameState {
        use crate::job::JobRegistry;
        use crate::module::ModuleRegistry;
        use crate::research::TechRegistry;
        let t = Tuning::default();
        let jobs = JobRegistry(vec![]);
        let mods = ModuleRegistry(vec![]);
        let techs = TechRegistry(vec![]);
        let world = crate::state::World {
            tuning: &t,
            jobs: &jobs,
            modules: &mods,
            techs: &techs,
        };
        let mut s = GameState::new(&world, 0xABCD);
        s.advance(&world, 17);
        s
    }

    #[test]
    fn round_trip_is_identity() {
        let s = sample();
        let encoded = encode(&s).unwrap();
        let decoded = decode(&encoded).unwrap();
        assert_eq!(s, decoded);
    }

    #[test]
    fn payload_is_url_safe() {
        let encoded = encode(&sample()).unwrap();
        assert!(!encoded.contains('+') && !encoded.contains('/') && !encoded.contains('='));
    }

    #[test]
    fn corrupted_base64_errors_cleanly() {
        assert_eq!(decode("not base64 !!!"), Err(SaveError::Base64));
    }

    #[test]
    fn flipped_byte_fails_checksum() {
        let mut encoded = encode(&sample()).unwrap();
        // Flip a char near the end (inside the compressed body).
        let mut chars: Vec<char> = encoded.chars().collect();
        let i = chars.len() - 2;
        chars[i] = if chars[i] == 'A' { 'B' } else { 'A' };
        encoded = chars.into_iter().collect();
        let err = decode(&encoded).unwrap_err();
        assert!(matches!(
            err,
            SaveError::Checksum
                | SaveError::Decompress
                | SaveError::Deserialize
                | SaveError::BadMagic
        ));
    }

    #[test]
    fn empty_input_is_bad_magic() {
        assert_eq!(decode(""), Err(SaveError::BadMagic));
    }

    #[test]
    fn rejects_oversized_decompression() {
        // A perfectly valid save, but decoded under a tiny output limit:
        // the real payload inflates past the cap and must be rejected, not OOM.
        let encoded = encode(&sample()).unwrap();
        assert_eq!(decode_with_limit(&encoded, 4), Err(SaveError::Decompress));
        // And the normal generous cap still round-trips fine:
        assert_eq!(decode(&encoded).unwrap(), sample());
    }

    #[test]
    fn wrong_version_is_rejected() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        // A valid save, then corrupt ONLY the framed schema-version field (bytes 4..8).
        // CRC covers only the compressed payload (bytes 12..), so the checksum still
        // matches and decode reaches the version dispatch.
        let encoded = encode(&sample()).unwrap();
        let mut framed = b64.decode(&encoded).unwrap();
        framed[4..8].copy_from_slice(&999u32.to_le_bytes());
        let tampered = b64.encode(&framed);
        assert_eq!(decode(&tampered), Err(SaveError::Version(999)));
    }

    #[test]
    fn v1_payload_is_rejected() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        // M0 saves carry schema version 1. Frame a valid v2 save, then set the
        // framed version field (bytes 4..8) to 1. CRC covers only the compressed
        // body (bytes 12..), so the checksum still matches and decode reaches the
        // version dispatch, which must cleanly reject (not crash / mis-deserialize).
        let encoded = encode(&sample()).unwrap();
        let mut framed = b64.decode(&encoded).unwrap();
        framed[4..8].copy_from_slice(&1u32.to_le_bytes());
        let tampered = b64.encode(&framed);
        assert_eq!(decode(&tampered), Err(SaveError::Version(1)));
    }

    #[test]
    fn v2_payload_is_rejected() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let encoded = encode(&sample()).unwrap();
        let mut framed = b64.decode(&encoded).unwrap();
        framed[4..8].copy_from_slice(&2u32.to_le_bytes());
        let tampered = b64.encode(&framed);
        assert_eq!(decode(&tampered), Err(SaveError::Version(2)));
    }

    #[test]
    fn v3_payload_is_rejected() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let encoded = encode(&sample()).unwrap();
        let mut framed = b64.decode(&encoded).unwrap();
        framed[4..8].copy_from_slice(&3u32.to_le_bytes());
        let tampered = b64.encode(&framed);
        assert_eq!(decode(&tampered), Err(SaveError::Version(3)));
    }

    #[test]
    fn v4_payload_is_rejected() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let encoded = encode(&sample()).unwrap();
        let mut framed = b64.decode(&encoded).unwrap();
        framed[4..8].copy_from_slice(&4u32.to_le_bytes());
        let tampered = b64.encode(&framed);
        assert_eq!(decode(&tampered), Err(SaveError::Version(4)));
    }

    #[test]
    fn v5_payload_is_rejected() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let encoded = encode(&sample()).unwrap();
        let mut framed = b64.decode(&encoded).unwrap();
        framed[4..8].copy_from_slice(&5u32.to_le_bytes());
        let tampered = b64.encode(&framed);
        assert_eq!(decode(&tampered), Err(SaveError::Version(5)));
    }

    #[test]
    fn v6_payload_is_rejected() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let encoded = encode(&sample()).unwrap();
        let mut framed = b64.decode(&encoded).unwrap();
        framed[4..8].copy_from_slice(&6u32.to_le_bytes());
        let tampered = b64.encode(&framed);
        assert_eq!(decode(&tampered), Err(SaveError::Version(6)));
    }

    #[test]
    fn v7_payload_is_rejected() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let encoded = encode(&sample()).unwrap();
        let mut framed = b64.decode(&encoded).unwrap();
        framed[4..8].copy_from_slice(&7u32.to_le_bytes());
        let tampered = b64.encode(&framed);
        assert_eq!(decode(&tampered), Err(SaveError::Version(7)));
    }

    #[test]
    fn v8_payload_is_rejected() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let encoded = encode(&sample()).unwrap();
        let mut framed = b64.decode(&encoded).unwrap();
        framed[4..8].copy_from_slice(&8u32.to_le_bytes());
        let tampered = b64.encode(&framed);
        assert_eq!(decode(&tampered), Err(SaveError::Version(8)));
    }

    #[test]
    fn v9_payload_is_rejected() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let encoded = encode(&sample()).unwrap();
        let mut framed = b64.decode(&encoded).unwrap();
        framed[4..8].copy_from_slice(&9u32.to_le_bytes());
        let tampered = b64.encode(&framed);
        assert_eq!(decode(&tampered), Err(SaveError::Version(9)));
    }
}
