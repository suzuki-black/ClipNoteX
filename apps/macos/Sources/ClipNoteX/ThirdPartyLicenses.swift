// ThirdPartyLicenses.swift — bundled list of third-party software.
//
// As of v0.4 this is hand-maintained (rebuild manually when adding deps).
// In v1.0 we'll wire `cargo-about generate` into CI and bundle a JSON.
//
// Sources of truth:
//   - Cargo workspace deps (see top-level Cargo.toml)
//   - Swift dependencies (none — pure system frameworks)

import Foundation

struct LicenseEntry {
    let name: String
    let version: String
    let license: String
    let url: String
}

enum ThirdPartyLicenses {

    static let all: [LicenseEntry] = [
        // --- Rust ecosystem ---
        .init(name: "tokio",            version: "1.x",     license: "MIT",          url: "https://github.com/tokio-rs/tokio"),
        .init(name: "redb",             version: "2.x",     license: "MIT/Apache-2.0", url: "https://github.com/cberner/redb"),
        .init(name: "chacha20poly1305", version: "0.10.x",  license: "MIT/Apache-2.0", url: "https://github.com/RustCrypto/AEADs"),
        .init(name: "argon2",           version: "0.5.x",   license: "MIT/Apache-2.0", url: "https://github.com/RustCrypto/password-hashes"),
        .init(name: "blake3",           version: "1.x",     license: "CC0-1.0/Apache-2.0", url: "https://github.com/BLAKE3-team/BLAKE3"),
        .init(name: "zstd",             version: "0.13.x",  license: "MIT/Apache-2.0", url: "https://github.com/gyscos/zstd-rs"),
        .init(name: "serde",            version: "1.x",     license: "MIT/Apache-2.0", url: "https://github.com/serde-rs/serde"),
        .init(name: "serde_json",       version: "1.x",     license: "MIT/Apache-2.0", url: "https://github.com/serde-rs/json"),
        .init(name: "bincode",          version: "1.3.x",   license: "MIT",          url: "https://github.com/bincode-org/bincode"),
        .init(name: "ulid",             version: "1.x",     license: "MIT",          url: "https://github.com/dylanhart/ulid-rs"),
        .init(name: "chrono",           version: "0.4.x",   license: "MIT/Apache-2.0", url: "https://github.com/chronotope/chrono"),
        .init(name: "thiserror",        version: "1.x",     license: "MIT/Apache-2.0", url: "https://github.com/dtolnay/thiserror"),
        .init(name: "anyhow",           version: "1.x",     license: "MIT/Apache-2.0", url: "https://github.com/dtolnay/anyhow"),
        .init(name: "async-trait",      version: "0.1.x",   license: "MIT/Apache-2.0", url: "https://github.com/dtolnay/async-trait"),
        .init(name: "sqlformat",        version: "0.3.x",   license: "MIT/Apache-2.0", url: "https://github.com/shssoichiro/sqlformat-rs"),
        .init(name: "tracing",          version: "0.1.x",   license: "MIT",          url: "https://github.com/tokio-rs/tracing"),
        .init(name: "tracing-subscriber", version: "0.3.x", license: "MIT",          url: "https://github.com/tokio-rs/tracing"),
        .init(name: "parking_lot",      version: "0.12.x",  license: "MIT/Apache-2.0", url: "https://github.com/Amanieu/parking_lot"),
        .init(name: "once_cell",        version: "1.x",     license: "MIT/Apache-2.0", url: "https://github.com/matklad/once_cell"),
        .init(name: "arc-swap",         version: "1.x",     license: "MIT/Apache-2.0", url: "https://github.com/vorner/arc-swap"),
        .init(name: "arboard",          version: "3.x",     license: "MIT/Apache-2.0", url: "https://github.com/1Password/arboard"),
        .init(name: "enigo",            version: "0.3.x",   license: "MIT",          url: "https://github.com/enigo-rs/enigo"),
        .init(name: "global-hotkey",    version: "0.6.x",   license: "Apache-2.0/MIT", url: "https://github.com/tauri-apps/global-hotkey"),
        .init(name: "keyring",          version: "3.x",     license: "MIT/Apache-2.0", url: "https://github.com/hwchen/keyring-rs"),
        .init(name: "image",            version: "0.25.x",  license: "MIT/Apache-2.0", url: "https://github.com/image-rs/image"),
        .init(name: "objc2",            version: "0.5.x",   license: "MIT",          url: "https://github.com/madsmtm/objc2"),
        .init(name: "objc2-app-kit",    version: "0.2.x",   license: "MIT",          url: "https://github.com/madsmtm/objc2"),
        .init(name: "objc2-foundation", version: "0.2.x",   license: "MIT",          url: "https://github.com/madsmtm/objc2"),
        .init(name: "cbindgen",         version: "0.27.x",  license: "MPL-2.0",      url: "https://github.com/mozilla/cbindgen"),
        .init(name: "rand",             version: "0.8.x",   license: "MIT/Apache-2.0", url: "https://github.com/rust-random/rand"),
        .init(name: "zeroize",          version: "1.x",     license: "MIT/Apache-2.0", url: "https://github.com/RustCrypto/utils"),
        .init(name: "libc",             version: "0.2.x",   license: "MIT/Apache-2.0", url: "https://github.com/rust-lang/libc"),
    ]

    /// Plain-text representation, one line per entry.
    static func asPlainText() -> String {
        let header = "ClipNoteX depends on the following third-party software.\nAll components are used under their respective open-source licenses.\n\n"
        let body = all
            .sorted { $0.name < $1.name }
            .map { "• \($0.name) \($0.version)\n    License: \($0.license)\n    \($0.url)" }
            .joined(separator: "\n\n")
        return header + body
    }
}
