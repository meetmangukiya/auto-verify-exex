use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_primitives::{Address, Bytes};
use foundry_block_explorers::verify::{CodeFormat, VerifyContract};
use std::collections::HashMap;

struct Encoder {
    compiled_bytecode: Vec<u8>,
    source: String,
    code_format: CodeFormat,
    param_types: DynSolType,
    contract_name: String,
    compiler_version: String,
    optimizations_used: Option<String>,
    runs: Option<String>,
    evm_version: Option<String>,
    via_ir: Option<bool>,
}

impl Encoder {
    fn new(data: ContractDeployArgs) -> Self {
        Self {
            compiled_bytecode: data.init_code_without_args.to_vec(),
            param_types: data
                .param_types
                .as_str()
                .parse::<_>()
                .expect("Invalid param type"),
            source: data.source,
            code_format: data.code_format,
            contract_name: data.contract_name,
            compiler_version: data.compiler_version,
            optimizations_used: data.optimizations_used,
            runs: data.runs,
            evm_version: data.evm_version,
            via_ir: data.via_ir,
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

pub struct ContractDeployArgs {
    pub factory_selector: [u8; 4],
    pub init_code_without_args: Bytes,
    pub source: String,
    pub code_format: CodeFormat,
    pub param_types: String,
    pub contract_name: String,
    pub compiler_version: String,
    pub optimizations_used: Option<String>,
    pub runs: Option<String>,
    pub evm_version: Option<String>,
    pub via_ir: Option<bool>,
}

impl Matcher {
    pub fn new(data: Vec<(Address, Vec<ContractDeployArgs>)>) -> Self {
        let mut mapping = HashMap::new();
        for (address, data) in data {
            let mut inner = HashMap::new();
            for config in data {
                inner.insert(config.factory_selector, Encoder::new(config));
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

    pub fn get_verification_args(&self, address: Address, data: &Bytes) -> Option<VerifyContract> {
        if let Some(inner) = self.mapping.get(&address) {
            if let Some(encoder) = inner.get(&data[0..4]) {
                if let Some(decoded) = encoder.decode(data) {
                    println!("{:?}", decoded);
                    return Some(VerifyContract {
                        address,
                        source: encoder.source.clone(),
                        code_format: encoder.code_format,
                        contract_name: encoder.contract_name.clone(),
                        compiler_version: encoder.compiler_version.clone(),
                        optimization_used: encoder.optimizations_used.clone(),
                        runs: encoder.runs.clone(),
                        constructor_arguments: todo!(),
                        blockscout_constructor_arguments: None,
                        evm_version: encoder.evm_version,
                        via_ir: encoder.via_ir,
                        other: Default::default(),
                    });
                }
            }
        }
        None
    }
}
