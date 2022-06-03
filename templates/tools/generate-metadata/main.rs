extern crate contract;

extern "Rust" {
    fn __near_abi() -> near_sdk::__private::AbiRoot;
}

fn main() -> Result<(), std::io::Error> {
    let abi = unsafe { __near_abi() };
    let contents = serde_json::to_string_pretty(&abi)?;
    print!("{}", contents);
    Ok(())
}
