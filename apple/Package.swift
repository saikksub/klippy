// swift-tools-version: 5.9
import PackageDescription

// Everything under ./generated is produced by `make generate`:
//   generated/App              Event / ViewModel / Effect types (crux codegen)
//   generated/Shared/Shared    Swift bindings for CoreFfi (boltffi generate swift)
//   generated/Shared/SharedFFI C header for those bindings
//   generated/Shared/lib       libshared.a, the Rust core
let rustLib = "\(Context.packageDirectory)/generated/Shared/lib/libshared.a"

let package = Package(
    name: "Klippy",
    platforms: [.macOS(.v14)],
    dependencies: [
        .package(path: "generated/App"),
    ],
    targets: [
        .target(
            name: "SharedFFI",
            path: "generated/Shared/SharedFFI",
            publicHeadersPath: "include"
        ),
        .target(
            name: "Shared",
            dependencies: ["SharedFFI"],
            path: "generated/Shared/Shared",
            linkerSettings: [
                .unsafeFlags([rustLib]),
                // SQLCipher in the Rust core encrypts with CommonCrypto.
                .linkedFramework("Security"),
                .linkedFramework("CoreFoundation"),
            ]
        ),
        .executableTarget(
            name: "Klippy",
            dependencies: [
                "Shared",
                .product(name: "App", package: "App"),
            ]
        ),
        .testTarget(
            name: "KlippyTests",
            dependencies: ["Klippy", .product(name: "App", package: "App")]
        ),
    ]
)
