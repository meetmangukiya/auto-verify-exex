use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_primitives::{Address, Bytes};
use std::collections::HashMap;

struct Encoder {
    compiled_bytecode: Vec<u8>,
    param_types: DynSolType,
}

impl Encoder {
    fn new(compiled_bytecode: Vec<u8>, param_types: String) -> Self {
        Self {
            compiled_bytecode,
            param_types: param_types
                .as_str()
                .parse::<_>()
                .expect("Invalid param type"),
        }
    }

    fn decode(&self, data: &Bytes) -> Option<DynSolValue> {
        if data.len() >= self.compiled_bytecode.len() {
            let params = self
                .param_types
                .abi_decode_params(&data[self.compiled_bytecode.len()..]);
            if params.is_ok() {
                Some(params.unwrap())
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub struct Matcher {
    mapping: HashMap<Address, HashMap<[u8; 4], Encoder>>,
}

impl Matcher {
    pub fn new(data: Vec<(Address, Vec<([u8; 4], Bytes, String)>)>) -> Self {
        let mut mapping = HashMap::new();
        for (address, data) in data {
            let mut inner = HashMap::new();
            for (selector, bytecode, param_types) in data {
                inner.insert(selector, Encoder::new(bytecode.into(), param_types));
            }
            mapping.insert(address, inner);
        }
        Self { mapping }
    }

    pub fn get_constructor_args(&self, address: Address, data: &Bytes) -> Option<DynSolValue> {
        if let Some(inner) = self.mapping.get(&address) {
            if let Some(encoder) = inner.get(&data[0..4]) {
                if let Some(decoded) = encoder.decode(data) {
                    println!("{:?}", decoded);
                    return Some(decoded);
                }
            }
        }
        None
    }
}
