// Tests that currently fail when run against fancy-regex, see README.md.
//
// x2 tests check if a pattern matches against an input at the specified start/end positions.
// x3 tests have an additional argument which is the group number to check.


  // No match found
  x2("^a", "\na", 1, 2);

  // Compile failed: ParseError(1, InvalidEscape("\\O"))
  x2("$\\O", "bb\n", 2, 3);

  // Compile failed: ParseError(0, InvalidEscape("\\Z"))
  x2("\\Z", "", 0, 0);

  // Compile failed: ParseError(0, InvalidEscape("\\c"))
  x2("\\ca", "\001", 0, 1);

  // Compile failed: ParseError(0, InvalidEscape("\\C"))
  x2("\\C-b", "\002", 0, 1);

  // Compile failed: ParseError(0, InvalidEscape("\\c"))
  x2("\\c\\\\", "\034", 0, 1);

  // Compile failed: ParseError(2, InvalidEscape("\\c"))
  x2("q[\\c\\\\]", "q\034", 0, 2);

  // Compile failed: ParseError(1, InvalidBackref)
  x2("\\17", "\017", 0, 1);

  // No match found
  x2("(?x)  G (o O(?-x)oO) g L", "GoOoOgLe", 0, 7);

  // Compile failed: ParseError(2, InvalidBackref)
  x2("[\\044-\\047]", "\046", 0, 1);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Parse(Error { kind: ClassRangeInvalid, pattern: "[a-&&-a]", span: Span(Position(o: 1, l: 1, c: 2), Position(o: 4, l: 1, c: 5)) }) } }))
  x2("[a-&&-a]", "-", 0, 1);

  // Compile failed: ParseError(2, InvalidEscape("\\Z"))
  x2("\\A\\Z", "", 0, 0);

  // Compile failed: ParseError(3, InvalidEscape("\\Z"))
  x2("xyz\\Z", "xyz", 0, 3);

  // Compile failed: ParseError(1, InvalidEscape("\\Z"))
  x2("a\\Z", "a", 0, 1);

  // No match found
  x2("(?i:ss)", "\xc3\x9f", 0, 2);

  // No match found
  x2("(?i:ss)", "\xe1\xba\x9e", 0, 3);

  // No match found
  x2("(?i:xssy)", "x\xc3\x9fy", 0, 4);

  // No match found
  x2("(?i:xssy)", "x\xe1\xba\x9ey", 0, 5);

  // No match found
  x2("(?i:\xc3\x9f)", "ss", 0, 2);

  // No match found
  x2("(?i:\xc3\x9f)", "SS", 0, 2);

  // No match found
  x2("(?i:[\xc3\x9f])", "ss", 0, 2);

  // No match found
  x2("(?i:[\xc3\x9f])", "SS", 0, 2);

  // No match found
  x2("(?m:.)", "\n", 0, 1);

  // No match found
  x2("(?m:a.)", "a\n", 0, 2);

  // No match found
  x2("(?m:.b)", "a\nb", 1, 3);

  // Compile failed: ParseError(3, InvalidEscape("\\Z"))
  x2("a|b\\Z", "ba", 1, 2);

  // Compile failed: ParseError(3, InvalidEscape("\\Z"))
  x2("a|b\\Z", "b", 0, 1);

  // Match found at start 1 and end 2 (expected 0 and 2)
  x2("a(?i)b|c", "aC", 0, 2);

  // No match found
  x2("(?:ab)?{2}", "", 0, 0);

  // No match found
  x2("(?:ab)?{2}", "ababa", 0, 4);

  // No match found
  x2("(?:ab)*{0}", "ababa", 0, 0);

  // Match found at start 0 and end 2 (expected 0 and 5)
  x2("(?:ab){,}", "ab{,}", 0, 5);

  // No match found
  x2("(?:abc)+?{2}", "abcabcabc", 0, 6);

  // No match found
  x2("(abc)(?i:\\1)", "abcABC", 0, 6);

  // No match found
  x3("((?m:a.c))", "a\nc", 0, 3, 1);

  // Compile failed: CompileError(InvalidBackref)
  x2("(?:(?:\\1|z)(a))+$", "zaaa", 0, 4);

  // Compile failed: ParseError(3, InvalidEscape("\\Z"))
  x2("(a*\\Z)\\1", "a", 1, 1);

  // Compile failed: ParseError(4, InvalidEscape("\\Z"))
  x2(".(a*\\Z)\\1", "ba", 1, 2);

  // Compile failed: ParseError(3, InvalidEscape("\\g"))
  x2("(a)\\g<1>", "aa", 0, 2);

  // Compile failed: ParseError(13, InvalidEscape("\\g"))
  x2("(?<name_2>ab)\\g<name_2>", "abab", 0, 4);

  // Compile failed: ParseError(4, InvalidEscape("\\g"))
  x2("(?<=\\g<ab>)|-\\zEND (?<ab>XyZ)", "XyZ", 3, 3);

  // Compile failed: ParseError(7, InvalidEscape("\\g"))
  x2("(?<n>|a\\g<n>)+", "", 0, 0);

  // Compile failed: ParseError(8, InvalidEscape("\\g"))
  x2("(?<n>|\\(\\g<n>\\))+$", "()(())", 0, 6);

  // Compile failed: ParseError(0, InvalidEscape("\\g"))
  x3("\\g<n>(?<n>.){0}", "X", 0, 1, 1);

  // Compile failed: ParseError(0, InvalidEscape("\\g"))
  x2("\\g<n>(abc|df(?<n>.YZ){2,8}){0}", "XYZ", 0, 3);

  // Compile failed: ParseError(9, InvalidEscape("\\g"))
  x2("\\A(?<n>(a\\g<n>)|)\\z", "aaaa", 0, 4);

  // Compile failed: ParseError(6, InvalidEscape("\\g"))
  x2("(?<n>|\\g<m>\\g<n>)\\z|\\zEND (?<m>a|(b)\\g<m>)", "bbbbabba", 0, 8);

  // Compile failed: ParseError(15, InvalidEscape("\\g"))
  x3("(z)()()(?<_9>a)\\g<_9>", "zaa", 2, 3, 1);

  // No match found
  x2("(?:(?<x>)|(?<x>efg))\\k<x>", "", 0, 0);

  // No match found
  x2("(?:(?<n1>.)|(?<n1>..)|(?<n1>...)|(?<n1>....)|(?<n1>.....)|(?<n1>......)|(?<n1>.......)|(?<n1>........)|(?<n1>.........)|(?<n1>..........)|(?<n1>...........)|(?<n1>............)|(?<n1>.............)|(?<n1>..............))\\k<n1>$", "a-pyumpyum", 2, 10);

  // Compile failed: ParseError(11, InvalidEscape("\\g"))
  x2("(?<foo>a|\\(\\g<foo>\\))", "a", 0, 1);

  // Compile failed: ParseError(11, InvalidEscape("\\g"))
  x2("(?<foo>a|\\(\\g<foo>\\))", "((((((a))))))", 0, 13);

  // Compile failed: ParseError(11, InvalidEscape("\\g"))
  x3("(?<foo>a|\\(\\g<foo>\\))", "((((((((a))))))))", 0, 17, 1);

  // Compile failed: ParseError(0, InvalidEscape("\\g"))
  x2("\\g<bar>|\\zEND(?<bar>.*abc$)", "abcxxxabc", 0, 9);

  // Compile failed: ParseError(0, InvalidEscape("\\g"))
  x2("\\g<1>|\\zEND(.a.)", "bac", 0, 3);

  // Compile failed: ParseError(0, InvalidEscape("\\g"))
  x3("\\g<_A>\\g<_A>|\\zEND(.a.)(?<_A>.b.)", "xbxyby", 3, 6, 1);

  // Compile failed: ParseError(5, InvalidEscape("\\g"))
  x2("\\A(?:\\g<pon>|\\g<pan>|\\zEND  (?<pan>a|c\\g<pon>c)(?<pon>b|d\\g<pan>d))$", "cdcbcdc", 0, 7);

  // Compile failed: ParseError(9, InvalidEscape("\\g"))
  x2("\\A(?<n>|a\\g<m>)\\z|\\zEND (?<m>\\g<n>)", "aaaa", 0, 4);

  // Compile failed: ParseError(9, InvalidEscape("\\g"))
  x2("(?<n>(a|b\\g<n>c){3,5})", "baaaaca", 1, 5);

  // Compile failed: ParseError(9, InvalidEscape("\\g"))
  x2("(?<n>(a|b\\g<n>c){3,5})", "baaaacaaaaa", 0, 10);

  // Compile failed: ParseError(21, InvalidEscape("\\g"))
  x2("(?<pare>\\(([^\\(\\)]++|\\g<pare>)*+\\))", "((a))", 0, 5);

  // No match found
  x2("()*\\1", "", 0, 0);

  // No match found
  x2("(?:()|())*\\1\\2", "", 0, 0);

  // Compile failed: CompileError(InvalidBackref)
  x3("(?:\\1a|())*", "a", 0, 0, 1);

  // Compile failed: ParseError(16, InvalidEscape("\\Z"))
  x2("x((.)*)*x(?i:\\1)\\Z", "0x1x2x1X2", 1, 9);

  // No match found
  x2("(?:()|()|()|()|()|())*\\2\\5", "", 0, 0);

  // No match found
  x2("(?:()|()|()|(x)|()|())*\\2b\\5", "b", 0, 1);

  // Compile failed: ParseError(12, InvalidEscape("\\g"))
  x3("(\\(((?:[^(]|\\g<1>)*)\\))", "(abc)(abc)", 1, 4, 2);

  // Compile failed: ParseError(0, InvalidEscape("\\o"))
  x2("\\o{101}", "A", 0, 1);

  // Compile failed: ParseError(6, InvalidEscape("\\g"))
  x2("\\A(a|b\\g<1>c)\\k<1+3>\\z", "bbacca", 0, 6);

  // Compile failed: ParseError(10, InvalidEscape("\\g"))
  x2("(?i)\\A(a|b\\g<1>c)\\k<1+2>\\z", "bBACcbac", 0, 8);

  // No match found
  x2("(?i)(?<X>aa)|(?<X>bb)\\k<X>", "BBbb", 0, 4);

  // Compile failed: ParseError(5, InvalidGroupName)
  x2("(?:\\k'+1'B|(A)C)*", "ACAB", 0, 4);

  // Compile failed: ParseError(0, InvalidEscape("\\g"))
  x2("\\g<+2>(abc)(ABC){0}", "ABCabc", 0, 6);

  // Compile failed: ParseError(1, InvalidEscape("\\g"))
  x2("A\\g'0'|B()", "AAAAB", 0, 5);

  // Compile failed: ParseError(2, InvalidEscape("\\g"))
  x3("(A\\g'0')|B", "AAAAB", 0, 5, 1);

  // Compile failed: ParseError(10, GeneralParseError("expected conditional to be a backreference or at least an expression for when the condition is true"))
  x2("(a*)(?(-1))aa", "aaaaa", 0, 5);

  // Compile failed: ParseError(7, GeneralParseError("expected close paren"))
  x2("(a)(?(1+0)b|c)d", "abd", 0, 3);

  // Compile failed: ParseError(5, UnknownFlag("(?'"))
  x2("(?:(?'name'a)|(?'name'b))(?('name')c|d)e", "ace", 0, 3);

  // Compile failed: ParseError(5, UnknownFlag("(?'"))
  x2("(?:(?'name'a)|(?'name'b))(?('name')c|d)e", "bce", 0, 3);

  // Compile failed: ParseError(0, InvalidEscape("\\R"))
  x2("\\R", "\r\n", 0, 2);

  // Compile failed: ParseError(0, InvalidEscape("\\R"))
  x2("\\R", "\r", 0, 1);

  // Compile failed: ParseError(0, InvalidEscape("\\R"))
  x2("\\R", "\n", 0, 1);

  // Compile failed: ParseError(0, InvalidEscape("\\R"))
  x2("\\R", "\x0b", 0, 1);

  // Compile failed: ParseError(0, InvalidEscape("\\R"))
  x2("\\R", "\xc2\x85", 0, 2);

  // Compile failed: ParseError(0, InvalidEscape("\\N"))
  x2("\\N", "a", 0, 1);

  // Compile failed: ParseError(0, InvalidEscape("\\O"))
  x2("\\O", "a", 0, 1);

  // Compile failed: ParseError(0, InvalidEscape("\\O"))
  x2("\\O", "\n", 0, 1);

  // Compile failed: ParseError(4, InvalidEscape("\\O"))
  x2("(?m:\\O)", "\n", 0, 1);

  // Compile failed: ParseError(5, InvalidEscape("\\O"))
  x2("(?-m:\\O)", "\n", 0, 1);

  // No match found
  x2("(?:()|())*\\1", "abc", 0, 0);

  // No match found
  x2("(?:()|())*\\2", "abc", 0, 0);

  // No match found
  x2("(?:()|()|())*\\3\\1", "abc", 0, 0);

  // Compile failed: ParseError(9, InvalidEscape("\\g"))
  x2("(|(?:a(?:\\g'1')*))b|", "abc", 0, 2);

  // Compile failed: ParseError(14, InvalidEscape("\\g"))
  x2("((?<x>abc){0}a\\g<x>d)+", "aabcd", 0, 5);

  // Match found at start 0 and end 3 (expected 0 and 6)
  x2("(?<x>a)(?<x>b)(\\k<x>)+", "abbaab", 0, 6);

  // Compile failed: ParseError(8, InvalidEscape("\\g"))
  x2("(?<x>$|b\\g<x>)", "bbb", 0, 3);

  // Compile failed: ParseError(16, InvalidEscape("\\g"))
  x2("(?<x>(?(a)a|b)|c\\g<x>)", "cccb", 0, 4);

  // Compile failed: ParseError(1, InvalidEscape("\\o"))
  x2("[\\o{101}]", "A", 0, 1);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~)", "", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~)", "A", 0, 0);

  // Compile failed: ParseError(7, UnknownFlag("(?~"))
  x2("aaaaa(?~)", "aaaaaaaaaa", 0, 5);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~(?:|aaa))", "aaa", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~aaa|)", "aaa", 0, 0);

  // Compile failed: ParseError(3, UnknownFlag("(?~"))
  x2("a(?~(?~)).", "abcdefghijklmnopqrstuvwxyz", 0, 26);

  // Compile failed: ParseError(5, UnknownFlag("(?~"))
  x2("/\\*(?~\\*/)\\*/", "/* */ */", 0, 5);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~\\w+)zzzzz", "zzzzz", 0, 5);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~\\w*)zzzzz", "zzzzz", 0, 5);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~A.C|B)", "ABC", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~XYZ|ABC)a", "ABCa", 1, 4);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~XYZ|ABC)a", "aABCa", 0, 1);

  // Compile failed: ParseError(9, UnknownFlag("(?~"))
  x2("<[^>]*>(?~[<>])</[^>]*>", "<a>vvv</a>   <b>  </b>", 0, 10);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~ab)", "ccc\ndab", 0, 5);

  // Compile failed: ParseError(6, UnknownFlag("(?~"))
  x2("(?m:(?~ab))", "ccc\ndab", 0, 5);

  // Compile failed: ParseError(7, UnknownFlag("(?~"))
  x2("(?-m:(?~ab))", "ccc\ndab", 0, 5);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~abc)xyz", "xyz012345678901234567890123456789abc", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|78|\\d*)", "123456789", 0, 6);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|def|(?:abc|de|f){0,100})", "abcdedeabcfdefabc", 0, 11);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|ab|.*)", "ccc\nddd", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|ab|\\O*)", "ccc\ndab", 0, 5);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|ab|\\O{2,10})", "ccc\ndab", 0, 5);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|ab|\\O{1,10})", "ab", 1, 2);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|abc|\\O{1,10})", "abc", 1, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|ab|\\O{5,10})|abc", "abc", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|ab|\\O{1,10})", "cccccccccccab", 0, 10);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|aaa|)", "aaa", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~||a*)", "aaaaaa", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~||a*?)", "aaaaaa", 0, 0);

  // Compile failed: ParseError(5, UnknownFlag("(?~"))
  x2("(a)(?~|b|\\1)", "aaaaaa", 0, 2);

  // Compile failed: ParseError(5, UnknownFlag("(?~"))
  x2("(a)(?~|bb|(?:a\\1)*)", "aaaaaa", 0, 5);

  // Compile failed: ParseError(7, UnknownFlag("(?~"))
  x2("(b|c)(?~|abac|(?:a\\1)*)", "abababacabab", 1, 4);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|aaaaa|a*+)", "aaaaa", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|aaaaaa|a*+)b", "aaaaaab", 1, 7);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|abcd|(?>))", "zzzabcd", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|abc|a*?)", "aaaabc", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|abc)a*", "aaaaaabc", 0, 5);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|abc)a*z|aaaaaabc", "aaaaaabc", 0, 8);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|aaaaaa)a*", "aaaaaa", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|abc)aaaa|aaaabc", "aaaabc", 0, 6);

  // Compile failed: ParseError(5, UnknownFlag("(?~"))
  x2("(?>(?~|abc))aaaa|aaaabc", "aaaabc", 0, 6);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|)a", "a", 0, 1);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|a)(?~|)a", "a", 0, 1);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|a).*(?~|)a", "bbbbbbbbbbbbbbbbbbbba", 0, 21);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|abc).*(xyz|pqr)(?~|)abc", "aaaaxyzaaapqrabc", 0, 16);

  // Compile failed: ParseError(2, UnknownFlag("(?~"))
  x2("(?~|abc).*(xyz|pqr)(?~|)abc", "aaaaxyzaaaabcpqrabc", 11, 19);

  // No match found
  x2("\\xca\\xb8", "\xca\xb8", 0, 2);

  // Compile failed: ParseError(9, InvalidEscape("\\Z"))
  x2("むめも\\Z", "むめも", 0, 9);

  // Compile failed: ParseError(9, InvalidEscape("\\Z"))
  x2("かきく\\Z", "かきく\n", 0, 9);

  // No match found
  x2("(?m:よ.)", "よ\n", 0, 4);

  // No match found
  x2("(?m:.め)", "ま\nめ", 3, 7);

  // Compile failed: ParseError(7, InvalidEscape("\\Z"))
  x2("鬼|車\\Z", "車鬼", 3, 6);

  // Compile failed: ParseError(7, InvalidEscape("\\Z"))
  x2("鬼|車\\Z", "車", 0, 3);

  // Compile failed: ParseError(7, InvalidEscape("\\Z"))
  x2("鬼|車\\Z", "車\n", 0, 3);

  // No match found
  x2("(?:あい)?{2}", "", 0, 0);

  // No match found
  x2("(?:鬼車)?{2}", "鬼車鬼車鬼", 0, 12);

  // No match found
  x2("(?:鬼車)*{0}", "鬼車鬼車鬼", 0, 0);

  // Match found at start 0 and end 6 (expected 0 and 9)
  x2("(?:鬼車){,}", "鬼車{,}", 0, 9);

  // No match found
  x2("(?:かきく)+?{2}", "かきくかきくかきく", 0, 18);

  // No match found
  x3("((?m:あ.う))", "あ\nう", 0, 7, 1);

  // Compile failed: ParseError(5, InvalidEscape("\\Z"))
  x2("(あ*\\Z)\\1", "あ", 3, 3);

  // Compile failed: ParseError(6, InvalidEscape("\\Z"))
  x2(".(あ*\\Z)\\1", "いあ", 3, 6);

  // Compile failed: ParseError(16, InvalidEscape("\\g"))
  x2("(?<愚か>変|\\(\\g<愚か>\\))", "((((((変))))))", 0, 15);

  // Compile failed: ParseError(5, InvalidEscape("\\g"))
  x2("\\A(?:\\g<阿_1>|\\g<云_2>|\\z終了  (?<阿_1>観|自\\g<云_2>自)(?<云_2>在|菩薩\\g<阿_1>菩薩))$", "菩薩自菩薩自在自菩薩自菩薩", 0, 39);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Parse(Error { kind: ClassRangeInvalid, pattern: "[あ-&&-あ]", span: Span(Position(o: 1, l: 1, c: 2), Position(o: 6, l: 1, c: 5)) }) } }))
  x2("[あ-&&-あ]", "-", 0, 1);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "\\p{^Emoji}", span: Span(Position(o: 0, l: 1, c: 1), Position(o: 10, l: 1, c: 11)) }) } }))
  x2("\\p{^Emoji}", "\xEF\xBC\x93", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "\\p{Word}", span: Span(Position(o: 0, l: 1, c: 1), Position(o: 8, l: 1, c: 9)) }) } }))
  x2("\\p{Word}", "こ", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "[\\p{Word}]", span: Span(Position(o: 1, l: 1, c: 2), Position(o: 9, l: 1, c: 10)) }) } }))
  x2("[\\p{Word}]", "こ", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "[^\\p{^Word}]", span: Span(Position(o: 2, l: 1, c: 3), Position(o: 11, l: 1, c: 12)) }) } }))
  x2("[^\\p{^Word}]", "こ", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "[^\\p{^Word}&&\\p{ASCII}]", span: Span(Position(o: 2, l: 1, c: 3), Position(o: 11, l: 1, c: 12)) }) } }))
  x2("[^\\p{^Word}&&\\p{ASCII}]", "こ", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "[^\\p{^Word}&&\\p{ASCII}]", span: Span(Position(o: 2, l: 1, c: 3), Position(o: 11, l: 1, c: 12)) }) } }))
  x2("[^\\p{^Word}&&\\p{ASCII}]", "a", 0, 1);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "[^[\\p{^Word}]&&[\\p{ASCII}]]", span: Span(Position(o: 3, l: 1, c: 4), Position(o: 12, l: 1, c: 13)) }) } }))
  x2("[^[\\p{^Word}]&&[\\p{ASCII}]]", "こ", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "[^[\\p{ASCII}]&&[^\\p{Word}]]", span: Span(Position(o: 17, l: 1, c: 18), Position(o: 25, l: 1, c: 26)) }) } }))
  x2("[^[\\p{ASCII}]&&[^\\p{Word}]]", "こ", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "[^[\\p{^Word}]&&[^\\p{ASCII}]]", span: Span(Position(o: 3, l: 1, c: 4), Position(o: 12, l: 1, c: 13)) }) } }))
  x2("[^[\\p{^Word}]&&[^\\p{ASCII}]]", "こ", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "[^\\p{^Word}&&[^၊]]", span: Span(Position(o: 2, l: 1, c: 3), Position(o: 11, l: 1, c: 12)) }) } }))
  x2("[^\\p{^Word}&&[^\\x{104a}]]", "こ", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "[^[\\p{^Word}]&&[^၊]]", span: Span(Position(o: 3, l: 1, c: 4), Position(o: 12, l: 1, c: 13)) }) } }))
  x2("[^[\\p{^Word}]&&[^\\x{104a}]]", "こ", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "\\p{^Cntrl}", span: Span(Position(o: 0, l: 1, c: 1), Position(o: 10, l: 1, c: 11)) }) } }))
  x2("\\p{^Cntrl}", "こ", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "[\\p{^Cntrl}]", span: Span(Position(o: 1, l: 1, c: 2), Position(o: 11, l: 1, c: 12)) }) } }))
  x2("[\\p{^Cntrl}]", "こ", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "[^[\\p{^Cntrl}]&&[\\p{ASCII}]]", span: Span(Position(o: 3, l: 1, c: 4), Position(o: 13, l: 1, c: 14)) }) } }))
  x2("[^[\\p{^Cntrl}]&&[\\p{ASCII}]]", "こ", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?-W"))
  x2("(?-W:\\p{Word})", "こ", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?W"))
  x2("(?W:\\p{Word})", "k", 0, 1);

  // Compile failed: ParseError(2, UnknownFlag("(?-W"))
  x2("(?-W:[[:word:]])", "こ", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?-D"))
  x2("(?-D:\\p{Digit})", "３", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?-S"))
  x2("(?-S:\\p{Space})", "\xc2\x85", 0, 2);

  // Compile failed: ParseError(2, UnknownFlag("(?-P"))
  x2("(?-P:\\p{Word})", "こ", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?-W"))
  x2("(?-W:\\w)", "こ", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?-W"))
  x2("(?-W:\\w)", "k", 0, 1);

  // Compile failed: ParseError(2, UnknownFlag("(?W"))
  x2("(?W:\\w)", "k", 0, 1);

  // Compile failed: ParseError(2, UnknownFlag("(?W"))
  x2("(?W:\\W)", "こ", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?-W"))
  x2("(?-W:\\b)", "こ", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?-W"))
  x2("(?-W:\\b)", "h", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?W"))
  x2("(?W:\\b)", "h", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?W"))
  x2("(?W:\\B)", "こ", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?-P"))
  x2("(?-P:\\b)", "こ", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?-P"))
  x2("(?-P:\\b)", "h", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?P"))
  x2("(?P:\\b)", "h", 0, 0);

  // Compile failed: ParseError(2, UnknownFlag("(?P"))
  x2("(?P:\\B)", "こ", 0, 0);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "\\p{InBasicLatin}", span: Span(Position(o: 0, l: 1, c: 1), Position(o: 16, l: 1, c: 17)) }) } }))
  x2("\\p{InBasicLatin}", "\x41", 0, 1);

  // Compile failed: ParseError(1, InvalidEscape("\\Y"))
  x2(".\\Y\\O", "\x0d\x0a", 0, 2);

  // Compile failed: ParseError(1, InvalidEscape("\\Y"))
  x2(".\\Y.", "\x67\xCC\x88", 0, 3);

  // Compile failed: ParseError(0, InvalidEscape("\\y"))
  x2("\\y.\\Y.\\y", "\x67\xCC\x88", 0, 3);

  // Compile failed: ParseError(0, InvalidEscape("\\y"))
  x2("\\y.\\y", "\xEA\xB0\x81", 0, 3);

  // Compile failed: ParseError(2, InvalidEscape("\\Y"))
  x2("^.\\Y.\\Y.$", "\xE1\x84\x80\xE1\x85\xA1\xE1\x86\xA8", 0, 9);

  // Compile failed: ParseError(1, InvalidEscape("\\Y"))
  x2(".\\Y.", "\xE0\xAE\xA8\xE0\xAE\xBF", 0, 6);

  // Compile failed: ParseError(1, InvalidEscape("\\Y"))
  x2(".\\Y.", "\xE0\xB8\x81\xE0\xB8\xB3", 0, 6);

  // Compile failed: ParseError(1, InvalidEscape("\\Y"))
  x2(".\\Y.", "\xE0\xA4\xB7\xE0\xA4\xBF", 0, 6);

  // Compile failed: ParseError(2, InvalidEscape("\\Y"))
  x2("..\\Y.", "\xE3\x80\xB0\xE2\x80\x8D\xE2\xAD\x95", 0, 9);

  // Compile failed: ParseError(3, InvalidEscape("\\Y"))
  x2("...\\Y.", "\xE3\x80\xB0\xCC\x82\xE2\x80\x8D\xE2\xAD\x95", 0, 11);

  // Compile failed: ParseError(1, InvalidEscape("\\X"))
  x2("^\\X$", "\x0d\x0a", 0, 2);

  // Compile failed: ParseError(1, InvalidEscape("\\X"))
  x2("^\\X$", "\x67\xCC\x88", 0, 3);

  // Compile failed: ParseError(1, InvalidEscape("\\X"))
  x2("^\\X$", "\xE1\x84\x80\xE1\x85\xA1\xE1\x86\xA8", 0, 9);

  // Compile failed: ParseError(1, InvalidEscape("\\X"))
  x2("^\\X$", "\xE0\xAE\xA8\xE0\xAE\xBF", 0, 6);

  // Compile failed: ParseError(1, InvalidEscape("\\X"))
  x2("^\\X$", "\xE0\xB8\x81\xE0\xB8\xB3", 0, 6);

  // Compile failed: ParseError(1, InvalidEscape("\\X"))
  x2("^\\X$", "\xE0\xA4\xB7\xE0\xA4\xBF", 0, 6);

  // Compile failed: ParseError(1, InvalidEscape("\\X"))
  x2("h\\Xllo", "ha\xCC\x80llo", 0, 7);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{g})\\yabc\\y", "abc", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{g})\\y\\X\\y", "abc", 0, 1);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\yabc\\y", "abc", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "\r\n", 0, 2);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "\x0cz", 0, 1);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "q\x0c", 0, 1);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "\xE2\x80\x8D\xE2\x9D\x87", 0, 6);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "\x20\x20", 0, 2);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "a\xE2\x80\x8D", 0, 4);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\y\\X\\y", "abc", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\y\\X\\y", "v\xCE\x87w", 0, 4);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\y\\X\\y", "\xD7\x93\x27", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\y\\X\\y", "\xD7\x93\x22\xD7\x93", 0, 5);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "14 45", 0, 2);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "a14", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "832e", 0, 4);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "8\xEF\xBC\x8C\xDB\xB0", 0, 6);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\y\\X\\y", "ケン", 0, 6);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\y\\X\\y", "ケン\xE2\x80\xAFタ", 0, 12);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\y\\X\\y", "\x21\x23", 0, 1);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\y\\X\\y", "山ア", 0, 3);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "3.14", 0, 4);

  // Compile failed: ParseError(2, UnknownFlag("(?y"))
  x2("(?y{w})\\X", "3 14", 0, 1);

  // Compile failed: ParseError(2, InvalidHex)
  x2("\\x1", "\x01", 0, 1);

  // Compile failed: ParseError(10, TargetNotRepeatable)
  x2("((?()0+)+++(((0\\g<0>)0)|())++++((?(1)(0\\g<0>))++++++0*())++++((?(1)(0\\g<1>)+)++++++++++*())++++((?(1)((0)\\g<0>)+)++())+0++*+++(((0\\g<0>))*())++++((?(1)(0\\g<0>)+)++++++++++*|)++++*+++((?(1)((0)\\g<0>)+)+++++++++())++*|)++++((?()0))|", "abcde", 0, 0);

  // Compile failed: ParseError(9, TargetNotRepeatable)
  x2("(?:[ab]|(*MAX{2}).)*", "abcbaaccaaa", 0, 7);

  // Compile failed: ParseError(4, TargetNotRepeatable)
  x2("(?:(*COUNT[AB]{X})[ab]|(*COUNT[CD]{X})[cd])*(*CMP{AB,<,CD})",
     "abababcdab", 5, 8);

  // Compile failed: ParseError(3, TargetNotRepeatable)
  x2("(?(?{....})123|456)", "123", 0, 3);

  // Compile failed: ParseError(3, TargetNotRepeatable)
  x2("(?(*FAIL)123|456)", "456", 0, 3);

  // Compile failed: ParseError(0, InvalidEscape("\\g"))
  x2("\\g'0'++{,0}",   "abcdefgh", 0, 0);

  // Compile failed: ParseError(0, InvalidEscape("\\g"))
  x2("\\g'0'++{,0}?",  "abcdefgh", 0, 0);

  // Compile failed: ParseError(0, InvalidEscape("\\g"))
  x2("\\g'0'++{,0}b",  "abcdefgh", 1, 2);

  // Compile failed: ParseError(0, InvalidEscape("\\g"))
  x2("\\g'0'++{,0}?def", "abcdefgh", 3, 6);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Parse(Error { kind: RepetitionCountInvalid, pattern: "a{3,2}b", span: Span(Position(o: 1, l: 1, c: 2), Position(o: 6, l: 1, c: 7)) }) } }))
  x2("a{3,2}b", "aaab", 0, 4);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Parse(Error { kind: RepetitionCountInvalid, pattern: "a{3,2}b", span: Span(Position(o: 1, l: 1, c: 2), Position(o: 6, l: 1, c: 7)) }) } }))
  x2("a{3,2}b", "aaaab", 1, 5);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Parse(Error { kind: RepetitionCountInvalid, pattern: "a{3,2}b", span: Span(Position(o: 1, l: 1, c: 2), Position(o: 6, l: 1, c: 7)) }) } }))
  x2("a{3,2}b", "aab", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Parse(Error { kind: RepetitionCountInvalid, pattern: "a{3,2}?", span: Span(Position(o: 1, l: 1, c: 2), Position(o: 7, l: 1, c: 8)) }) } }))
  x2("a{3,2}?", "", 0, 0);

  // No match found
  x2("a{2,3}+a", "aaa", 0, 3);

  // Compile failed: CompileError(InnerError(BuildError { kind: Syntax { pid: PatternID(0), err: Translate(Error { kind: UnicodePropertyNotFound, pattern: "\\p{In_Enclosed_CJK_Letters_and_Months}", span: Span(Position(o: 0, l: 1, c: 1), Position(o: 38, l: 1, c: 39)) }) } }))
  x2("\\p{In_Enclosed_CJK_Letters_and_Months}", "\xe3\x8b\xbf", 0, 3);
