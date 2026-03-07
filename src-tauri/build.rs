fn main() {
  // Ensure sources.json is included in rebuild triggers
  println!("cargo:rerun-if-changed=src/sources.json");
  tauri_build::build()
}

