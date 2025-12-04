import XCTest
import SwiftTreeSitter
import PARSER_CLASS_NAME

final class PARSER_CLASS_NAMETests: XCTestCase {
    func testCanLoadGrammar() throws {
        let parser = Parser()
        let language = Language(language: tree_sitter_LOWER_PARSER_NAME())
        XCTAssertNoThrow(try parser.setLanguage(language),
                         "Error loading TITLE_PARSER_NAME grammar")
    }
}
