use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=HAVE_SHARED_GAME_DIR");
    if let Ok(dir) = env::var("HAVE_SHARED_GAME_DIR") {
        println!("cargo:rustc-env=HAVE_SHARED_GAME_DIR={dir}");
    }
}
