/// Shared call payload envelope used by the host and contracts.
pub use blocky_sdk::CallEnvelope;

use thiserror::Error;

/// Errors returned when decoding structured contract call input.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CallAbiError {
    #[error("call envelope is empty")]
    Empty,
    #[error("failed to decode call envelope: {0}")]
    Decode(String),
}

/// Decodes a Borsh-encoded [`CallEnvelope`] and rejects empty payloads.
///
/// # Arguments
/// - `bytes`: Raw input bytes passed to a contract call.
///
/// # Returns
/// The decoded call envelope, or a validation/decoding error.
pub fn decode_checked(bytes: &[u8]) -> Result<CallEnvelope, CallAbiError> {
    if bytes.is_empty() {
        return Err(CallAbiError::Empty);
    }

    CallEnvelope::decode(bytes).map_err(CallAbiError::Decode)
}

#[cfg(test)]
mod tests {
    use super::{CallAbiError, CallEnvelope, decode_checked};

    #[test]
    fn round_trips_borsh_encoded_envelope() {
        let envelope = CallEnvelope::new("set", vec![1, 2, 3]);
        let encoded = envelope.encode().unwrap();

        let decoded = decode_checked(&encoded).unwrap();

        assert_eq!(decoded, envelope);
    }

    #[test]
    fn rejects_empty_payload() {
        assert_eq!(decode_checked(&[]).unwrap_err(), CallAbiError::Empty);
    }
}
