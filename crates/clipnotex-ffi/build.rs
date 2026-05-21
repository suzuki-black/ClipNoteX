//! Build script: regenerate `include/ClipNoteX.h` from FFI surface via cbindgen.
//!
//! Output:
//!   crates/clipnotex-ffi/include/ClipNoteX.h
//!
//! Swift プロジェクトはこのヘッダを bridging header としてインポートする。

use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = crate_dir.join("include");
    std::fs::create_dir_all(&out_dir).ok();
    let out_file = out_dir.join("ClipNoteX.h");

    // Only emit if cbindgen succeeds — failure is non-fatal so cargo build
    // works even when the header is unchanged.
    match cbindgen::generate(&crate_dir) {
        Ok(bindings) => {
            bindings.write_to_file(&out_file);
            println!("cargo:warning=ClipNoteX.h regenerated at {}", out_file.display());
        }
        Err(e) => {
            println!("cargo:warning=cbindgen skipped: {e}");
        }
    }

    // ヘッダ生成設定ファイル/ ソースを変更したら再実行
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/api.rs");
    println!("cargo:rerun-if-changed=src/state.rs");
    println!("cargo:rerun-if-changed=src/strings.rs");
    println!("cargo:rerun-if-changed=src/runtime.rs");
    println!("cargo:rerun-if-changed=src/errors.rs");
}
