use borsh::{BorshDeserialize, BorshSerialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct CallEnvelope {
    pub method: String,
    pub args: Vec<u8>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CallAbiError {
    #[error("call envelope is empty")]
    Empty,
    #[error("failed to decode call envelope: {0}")]
    Decode(String),
}

impl CallEnvelope {
    pub fn new(method: impl Into<String>, args: Vec<u8>) -> Self {
        Self {
            method: method.into(),
            args,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        borsh::to_vec(self).expect("call envelope serialization should succeed")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CallAbiError> {
        if bytes.is_empty() {
            return Err(CallAbiError::Empty);
        }

        borsh::from_slice(bytes).map_err(|error| CallAbiError::Decode(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{CallAbiError, CallEnvelope};

    #[test]
    fn round_trips_borsh_encoded_envelope() {
        let envelope = CallEnvelope::new("set", vec![1, 2, 3]);
        let encoded = envelope.encode();

        let decoded = CallEnvelope::decode(&encoded).unwrap();

        assert_eq!(decoded, envelope);
    }

    #[test]
    fn rejects_empty_payload() {
        assert_eq!(CallEnvelope::decode(&[]).unwrap_err(), CallAbiError::Empty);
    }
}
