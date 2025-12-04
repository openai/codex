// swift-tools-version:5.3
import PackageDescription

let package = Package(
    name: "TreeSitterBash",
    products: [
        .library(name: "TreeSitterBash", targets: ["TreeSitterBash"])
    ],
    dependencies: [
        .package(
            name: "SwiftTreeSitter",
            url: "https://github.com/tree-sitter/swift-tree-sitter",
            from: "0.25.0"
        )
    ],
    targets: [
        .target(
            name: "TreeSitterBash",
            dependencies: [],
            path: ".",
            sources: [
                "src/parser.c",
                "src/scanner.c",
            ],
            resources: [
                .copy("queries")
            ],
            publicHeadersPath: "bindings/swift",
            cSettings: [.headerSearchPath("src")]
        ),
        .testTarget(
            name: "TreeSitterBashTests",
            dependencies: [
                "SwiftTreeSitter",
                "TreeSitterBash",
            ],
            path: "bindings/swift/TreeSitterBashTests"
        ),
    ],
    cLanguageStandard: .c11
)
