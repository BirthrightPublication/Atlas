fn main() {
    println!("cargo:rerun-if-env-changed=ATLAS_UPDATE_PUBLIC_KEY");
    println!("cargo:rerun-if-env-changed=ATLAS_UPDATE_ENDPOINT");
    tauri_build::build();
}
