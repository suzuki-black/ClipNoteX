// swift-tools-version: 5.9
//
// ClipNoteX macOS app — Swift Package Manager manifest.
//
// Build:
//   1) cargo build --release -p clipnotex-ffi
//      → produces target/release/libclipnotex_ffi.a
//   2) swift build -c release
//      → links against the static library above and the bridging header
//
// The eventual deliverable is an .app bundle. SPM produces a CLI executable;
// a thin Xcode project (or Makefile) wraps it into Contents/MacOS/ClipNoteX.

import PackageDescription

let package = Package(
    name: "ClipNoteX",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "ClipNoteX", targets: ["ClipNoteX"]),
    ],
    targets: [
        // C module that exposes the Rust FFI header.
        .systemLibrary(
            name: "ClipNoteXCore",
            path: "Sources/ClipNoteXCore"
        ),
        // Swift app target.
        .executableTarget(
            name: "ClipNoteX",
            dependencies: ["ClipNoteXCore"],
            path: "Sources/ClipNoteX",
            linkerSettings: [
                .linkedLibrary("clipnotex_ffi"),
                .unsafeFlags([
                    // Static library produced by cargo.
                    "-L../../target/release",
                    "-L../../target/debug",
                ]),
                // Frameworks required by the Rust crates' macOS code.
                .linkedFramework("AppKit"),
                .linkedFramework("Carbon"),
                .linkedFramework("CoreFoundation"),
                .linkedFramework("Security"),
            ]
        ),
    ]
)
