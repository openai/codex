// swift-tools-version:5.3

import Foundation
import PackageDescription

var sources = ["src/parser.c"]
if FileManager.default.fileExists(atPath: "src/scanner.c") {
    sources.append("src/scanner.c")
}

let package = Package(
    name: "PARSER_CLASS_NAME",
    products: [
        .library(name: "PARSER_CLASS_NAME", targets: ["PARSER_CLASS_NAME"]),
    ],
    dependencies: [
        .package(name: "SwiftTreeSitter", url: "https://github.com/tree-sitter/swift-tree-sitter", from: "0.9.0"),
    ],
    targets: [
        .target(
            name: "PARSER_CLASS_NAME",
            dependencies: [],
            path: ".",
            sources: sources,
            resources: [
                .copy("queries")
            ],
            publicHeadersPath: "bindings/swift",
            cSettings: [.headerSearchPath("src")]
        ),
        .testTarget(
            name: "PARSER_CLASS_NAMETests",
            dependencies: [
                "SwiftTreeSitter",
                "PARSER_CLASS_NAME",
            ],
            path: "bindings/swift/PARSER_CLASS_NAMETests"
        )
    ],
    cLanguageStandard: .c11
)
