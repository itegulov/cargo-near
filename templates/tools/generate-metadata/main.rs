extern crate contract;

extern "Rust" {
    ::extern_function_defs::
}

fn main() -> Result<(), std::io::Error> {
    let root_abis = ::abi_calls::;
    let combined_root_abi = near_sdk::__private::AbiRoot::combine(root_abis);
    let contents = serde_json::to_string_pretty(&combined_root_abi)?;
    print!("{}", contents);
    Ok(())
}
