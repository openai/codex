import XCTest
import SwiftTreeSitter
import TreeSitterBash

final class TreeSitterBashTests: XCTestCase {
    func testCanLoadGrammar() throws {
        let parser = Parser()
        let language = Language(language: tree_sitter_bash())
        XCTAssertNoThrow(try parser.setLanguage(language),
                         "Error loading Bash grammar")
    }
}
