//! Save codec: postcard → deflate → frame(magic + crc32) → base64url.
//! Pure and headless-testable. Powers both localStorage and shareable links.

use crate::state::{GameState, SAVE_VERSION};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;

const MAGIC: &[u8; 4] = b"CSV1";

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

/// Serialize → compress → frame(magic + crc32) → base64url.
pub fn encode(state: &GameState) -> Result<String, SaveError> {
    let bytes = postcard::to_allocvec(state).map_err(|_| SaveError::Serialize)?;
    let compressed = miniz_oxide::deflate::compress_to_vec(&bytes, 6);
    let crc = crc32fast::hash(&compressed);
    let mut framed = Vec::with_capacity(8 + compressed.len());
    framed.extend_from_slice(MAGIC);
    framed.extend_from_slice(&crc.to_le_bytes());
    framed.extend_from_slice(&compressed);
    Ok(B64.encode(framed))
}

/// Reverse of `encode`, validating magic, checksum, and version.
pub fn decode(s: &str) -> Result<GameState, SaveError> {
    let framed = B64.decode(s.trim()).map_err(|_| SaveError::Base64)?;
    if framed.len() < 8 || &framed[0..4] != MAGIC {
        return Err(SaveError::BadMagic);
    }
    let crc = u32::from_le_bytes([framed[4], framed[5], framed[6], framed[7]]);
    let compressed = &framed[8..];
    if crc32fast::hash(compressed) != crc {
        return Err(SaveError::Checksum);
    }
    let bytes =
        miniz_oxide::inflate::decompress_to_vec(compressed).map_err(|_| SaveError::Decompress)?;
    let state: GameState = postcard::from_bytes(&bytes).map_err(|_| SaveError::Deserialize)?;
    if state.version != SAVE_VERSION {
        return Err(SaveError::Version(state.version));
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Tuning;

    fn sample() -> GameState {
        let mut s = GameState::new(&Tuning::default(), 0xABCD);
        s.advance(&Tuning::default(), 17);
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
}
