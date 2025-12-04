import XCTest
import SwiftTreeSitter
import TreeSitterJava

final class TreeSitterJavaTests: XCTestCase {
    func testCanLoadGrammar() throws {
        let parser = Parser()
        let language = Language(language: tree_sitter_java())
        XCTAssertNoThrow(try parser.setLanguage(language),
                         "Error loading Java grammar")
    }
}
