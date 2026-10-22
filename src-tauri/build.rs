fn main() {
    println!("cargo:rerun-if-env-changed=ATLAS_UPDATE_PUBLIC_KEY");
    println!("cargo:rerun-if-env-changed=ATLAS_UPDATE_ENDPOINT");
    println!("cargo:rerun-if-env-changed=ATLAS_UPDATE_TEST_CA_PEM");
    tauri_build::build();
}
