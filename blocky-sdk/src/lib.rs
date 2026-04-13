use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct CallEnvelope {
    pub method: String,
    pub args: Vec<u8>,
}

impl CallEnvelope {
    pub fn decode(bytes: &[u8]) -> Result<Self, std::io::Error> {
        borsh::from_slice(bytes)
    }
}

pub fn input() -> Vec<u8> {
    let len = unsafe { env::input_len() };
    if len <= 0 {
        return Vec::new();
    }

    let mut bytes = vec![0_u8; len as usize];
    let written = unsafe { env::read_input(bytes.as_mut_ptr(), len) };
    if written <= 0 {
        return Vec::new();
    }

    bytes.truncate(written as usize);
    bytes
}

pub fn call_envelope() -> Result<CallEnvelope, std::io::Error> {
    CallEnvelope::decode(&input())
}

pub fn decode_args<T: BorshDeserialize>() -> Result<T, std::io::Error> {
    let envelope = call_envelope()?;
    borsh::from_slice(&envelope.args)
}

pub mod env {
    unsafe extern "C" {
        pub fn input_len() -> i32;
        pub fn read_input(out_ptr: *mut u8, out_len: i32) -> i32;
    }
}

#[cfg(test)]
mod tests {
    use super::{CallEnvelope, decode_args};
    use borsh::{BorshDeserialize, BorshSerialize};

    #[derive(Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
    struct SetArgs {
        value: u64,
    }

    #[test]
    fn decodes_envelope() {
        let envelope = CallEnvelope {
            method: "set".to_string(),
            args: borsh::to_vec(&SetArgs { value: 7 }).unwrap(),
        };

        let decoded = CallEnvelope::decode(&borsh::to_vec(&envelope).unwrap()).unwrap();

        assert_eq!(decoded.method, "set");
    }

    #[test]
    fn decodes_typed_args_from_envelope_payload() {
        let envelope = CallEnvelope {
            method: "set".to_string(),
            args: borsh::to_vec(&SetArgs { value: 11 }).unwrap(),
        };
        let encoded = borsh::to_vec(&envelope).unwrap();
        let decoded = CallEnvelope::decode(&encoded).unwrap();
        let args: SetArgs = borsh::from_slice(&decoded.args).unwrap();

        assert_eq!(args, SetArgs { value: 11 });
        let _ = decode_args::<SetArgs>;
    }
}
