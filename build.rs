#[path = "src/cmudict.rs"]
mod cmudict;

fn main() {
    println!("cargo:rerun-if-changed=data/cmudict/cmudict.dict");
    println!("cargo:rerun-if-changed=src/cmudict.rs");
    let source = std::fs::read_to_string("data/cmudict/cmudict.dict").expect("read CMUdict source");
    let converted = cmudict::convert(&source).expect("valid CMUdict source");
    let path =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output directory"));
    std::fs::write(path.join("cmudict.txt"), converted).expect("write generated dictionary");
}
