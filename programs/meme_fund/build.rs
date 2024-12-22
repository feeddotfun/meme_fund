use std::env;
use std::fs;
use std::path::Path;
use serde_json::Value;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("pump_idl.rs");
    let idl_path = Path::new("idl/pump.json");
    
    let idl_content = fs::read_to_string(idl_path).expect("Failed to read IDL file");
    let idl: Value = serde_json::from_str(&idl_content).expect("Failed to parse IDL");
    
    let create_ix = idl["instructions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|ix| ix["name"] == "create")
        .expect("Create instruction not found");
    
    let create_discriminator = create_ix["discriminator"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect::<Vec<u8>>();

    let buy_ix = idl["instructions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|ix| ix["name"] == "buy")
        .expect("Buy instruction not found");
    
    let buy_discriminator = buy_ix["discriminator"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect::<Vec<u8>>();
    
    let idl_as_rust = format!(
        "pub const PUMP_IDL: &str = r#\"{}\"#;\n\
         pub const CREATE_DISCRIMINATOR: [u8; 8] = {:?};\n\
         pub const BUY_DISCRIMINATOR: [u8; 8] = {:?};",
        idl_content, create_discriminator, buy_discriminator
    );
    
    fs::write(&dest_path, idl_as_rust).unwrap();

    println!("cargo:rerun-if-changed=idl/pump.json");
}