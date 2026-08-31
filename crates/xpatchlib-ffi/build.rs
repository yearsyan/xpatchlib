fn main() {
    // The header is generated from source by scripts/generate-header.sh
    // (cbindgen); checked in so consumers never need cbindgen installed.
    println!("cargo:rerun-if-changed=include/xpatchlib.h");
}
