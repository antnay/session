use std::env;
use std::path::PathBuf;

fn main() {
    let home_dir = env::var("HOME").expect("HOME environment variable not set");

    let session_path = PathBuf::from(home_dir.clone())
        .join(".config")
        .join("session")
        .join("session.yml");

    println!("cargo:rustc-env=SESSION_PATH={}", session_path.display());
    println!("cargo:rustc-env=HOME_PATH={}", home_dir);
}
