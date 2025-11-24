use std::mem::transmute;

const DATA1: [(char, char); 277] = [
    ('\u{00C0}', 'A'), //  WITH GRAVE, LATIN CAPITAL LETTER
    ('\u{00C1}', 'A'), //  WITH ACUTE, LATIN CAPITAL LETTER
    ('\u{00C2}', 'A'), //  WITH CIRCUMFLEX, LATIN CAPITAL LETTER
    ('\u{00C3}', 'A'), //  WITH TILDE, LATIN CAPITAL LETTER
    ('\u{00C4}', 'A'), //  WITH DIAERESIS, LATIN CAPITAL LETTER
    ('\u{00C5}', 'A'), //  WITH RING ABOVE, LATIN CAPITAL LETTER
    ('\u{00C7}', 'C'), //  WITH CEDILLA, LATIN CAPITAL LETTER
    ('\u{00C8}', 'E'), //  WITH GRAVE, LATIN CAPITAL LETTER
    ('\u{00C9}', 'E'), //  WITH ACUTE, LATIN CAPITAL LETTER
    ('\u{00CA}', 'E'), //  WITH CIRCUMFLEX, LATIN CAPITAL LETTER
    ('\u{00CB}', 'E'), //  WITH DIAERESIS, LATIN CAPITAL LETTER
    ('\u{00CC}', 'I'), //  WITH GRAVE, LATIN CAPITAL LETTER
    ('\u{00CD}', 'I'), //  WITH ACUTE, LATIN CAPITAL LETTER
    ('\u{00CE}', 'I'), //  WITH CIRCUMFLEX, LATIN CAPITAL LETTER
    ('\u{00CF}', 'I'), //  WITH DIAERESIS, LATIN CAPITAL LETTER
    ('\u{00D1}', 'N'), //  WITH TILDE, LATIN CAPITAL LETTER
    ('\u{00D2}', 'O'), //  WITH GRAVE, LATIN CAPITAL LETTER
    ('\u{00D3}', 'O'), //  WITH ACUTE, LATIN CAPITAL LETTER
    ('\u{00D4}', 'O'), //  WITH CIRCUMFLEX, LATIN CAPITAL LETTER
    ('\u{00D5}', 'O'), //  WITH TILDE, LATIN CAPITAL LETTER
    ('\u{00D6}', 'O'), //  WITH DIAERESIS, LATIN CAPITAL LETTER
    ('\u{00D8}', 'O'), //  WITH STROKE, LATIN CAPITAL LETTER
    ('\u{00D9}', 'U'), //  WITH GRAVE, LATIN CAPITAL LETTER
    ('\u{00DA}', 'U'), //  WITH ACUTE, LATIN CAPITAL LETTER
    ('\u{00DB}', 'U'), //  WITH CIRCUMFLEX, LATIN CAPITAL LETTER
    ('\u{00DC}', 'U'), //  WITH DIAERESIS, LATIN CAPITAL LETTER
    ('\u{00DD}', 'Y'), //  WITH ACUTE, LATIN CAPITAL LETTER
    ('\u{00DF}', 's'), // , LATIN SMALL LETTER SHARP
    ('\u{00E0}', 'a'), //  WITH GRAVE, LATIN SMALL LETTER
    ('\u{00E1}', 'a'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{00E2}', 'a'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{00E3}', 'a'), //  WITH TILDE, LATIN SMALL LETTER
    ('\u{00E4}', 'a'), //  WITH DIAERESIS, LATIN SMALL LETTER
    ('\u{00E5}', 'a'), //  WITH RING ABOVE, LATIN SMALL LETTER
    ('\u{00E7}', 'c'), //  WITH CEDILLA, LATIN SMALL LETTER
    ('\u{00E8}', 'e'), //  WITH GRAVE, LATIN SMALL LETTER
    ('\u{00E9}', 'e'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{00EA}', 'e'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{00EB}', 'e'), //  WITH DIAERESIS, LATIN SMALL LETTER
    ('\u{00EC}', 'i'), //  WITH GRAVE, LATIN SMALL LETTER
    ('\u{00ED}', 'i'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{00EE}', 'i'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{00EF}', 'i'), //  WITH DIAERESIS, LATIN SMALL LETTER
    ('\u{00F1}', 'n'), //  WITH TILDE, LATIN SMALL LETTER
    ('\u{00F2}', 'o'), //  WITH GRAVE, LATIN SMALL LETTER
    ('\u{00F3}', 'o'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{00F4}', 'o'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{00F5}', 'o'), //  WITH TILDE, LATIN SMALL LETTER
    ('\u{00F6}', 'o'), //  WITH DIAERESIS, LATIN SMALL LETTER
    ('\u{00F8}', 'o'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{00F9}', 'u'), //  WITH GRAVE, LATIN SMALL LETTER
    ('\u{00FA}', 'u'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{00FB}', 'u'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{00FC}', 'u'), //  WITH DIAERESIS, LATIN SMALL LETTER
    ('\u{00FD}', 'y'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{00FF}', 'y'), //  WITH DIAERESIS, LATIN SMALL LETTER
    ('\u{0101}', 'a'), //  WITH MACRON, LATIN SMALL LETTER
    ('\u{0103}', 'a'), //  WITH BREVE, LATIN SMALL LETTER
    ('\u{0105}', 'a'), //  WITH OGONEK, LATIN SMALL LETTER
    ('\u{0107}', 'c'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{0109}', 'c'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{010B}', 'c'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{010D}', 'c'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{010F}', 'd'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{0111}', 'd'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{0113}', 'e'), //  WITH MACRON, LATIN SMALL LETTER
    ('\u{0115}', 'e'), //  WITH BREVE, LATIN SMALL LETTER
    ('\u{0117}', 'e'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{0119}', 'e'), //  WITH OGONEK, LATIN SMALL LETTER
    ('\u{011B}', 'e'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{011D}', 'g'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{011F}', 'g'), //  WITH BREVE, LATIN SMALL LETTER
    ('\u{0121}', 'g'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{0123}', 'g'), //  WITH CEDILLA, LATIN SMALL LETTER
    ('\u{0125}', 'h'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{0127}', 'h'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{0129}', 'i'), //  WITH TILDE, LATIN SMALL LETTER
    ('\u{012B}', 'i'), //  WITH MACRON, LATIN SMALL LETTER
    ('\u{012D}', 'i'), //  WITH BREVE, LATIN SMALL LETTER
    ('\u{012F}', 'i'), //  WITH OGONEK, LATIN SMALL LETTER
    ('\u{0130}', 'I'), //  WITH DOT ABOVE, LATIN CAPITAL LETTER
    ('\u{0131}', 'i'), // , LATIN SMALL LETTER DOTLESS
    ('\u{0135}', 'j'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{0137}', 'k'), //  WITH CEDILLA, LATIN SMALL LETTER
    ('\u{013A}', 'l'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{013C}', 'l'), //  WITH CEDILLA, LATIN SMALL LETTER
    ('\u{013E}', 'l'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{0140}', 'l'), //  WITH MIDDLE DOT, LATIN SMALL LETTER
    ('\u{0142}', 'l'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{0144}', 'n'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{0146}', 'n'), //  WITH CEDILLA, LATIN SMALL LETTER
    ('\u{0148}', 'n'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{014D}', 'o'), //  WITH MACRON, LATIN SMALL LETTER
    ('\u{014F}', 'o'), //  WITH BREVE, LATIN SMALL LETTER
    ('\u{0151}', 'o'), //  WITH DOUBLE ACUTE, LATIN SMALL LETTER
    ('\u{0155}', 'r'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{0157}', 'r'), //  WITH CEDILLA, LATIN SMALL LETTER
    ('\u{0159}', 'r'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{015B}', 's'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{015D}', 's'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{015F}', 's'), //  WITH CEDILLA, LATIN SMALL LETTER
    ('\u{0161}', 's'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{0163}', 't'), //  WITH CEDILLA, LATIN SMALL LETTER
    ('\u{0165}', 't'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{0167}', 't'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{0169}', 'u'), //  WITH TILDE, LATIN SMALL LETTER
    ('\u{016B}', 'u'), //  WITH MACRON, LATIN SMALL LETTER
    ('\u{016D}', 'u'), //  WITH BREVE, LATIN SMALL LETTER
    ('\u{016F}', 'u'), //  WITH RING ABOVE, LATIN SMALL LETTER
    ('\u{0171}', 'u'), //  WITH DOUBLE ACUTE, LATIN SMALL LETTER
    ('\u{0173}', 'u'), //  WITH OGONEK, LATIN SMALL LETTER
    ('\u{0175}', 'w'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{0177}', 'y'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER
    ('\u{0178}', 'Y'), //  WITH DIAERESIS, LATIN CAPITAL LETTER
    ('\u{017A}', 'z'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{017C}', 'z'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{017E}', 'z'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{017F}', 's'), // , LATIN SMALL LETTER LONG
    ('\u{0180}', 'b'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{0181}', 'B'), //  WITH HOOK, LATIN CAPITAL LETTER
    ('\u{0183}', 'b'), //  WITH TOPBAR, LATIN SMALL LETTER
    ('\u{0186}', 'O'), // , LATIN CAPITAL LETTER OPEN
    ('\u{0188}', 'c'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{0189}', 'D'), // , LATIN CAPITAL LETTER AFRICAN
    ('\u{018A}', 'D'), //  WITH HOOK, LATIN CAPITAL LETTER
    ('\u{018C}', 'd'), //  WITH TOPBAR, LATIN SMALL LETTER
    ('\u{018E}', 'E'), // , LATIN CAPITAL LETTER REVERSED
    ('\u{0190}', 'E'), // , LATIN CAPITAL LETTER OPEN
    ('\u{0192}', 'f'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{0193}', 'G'), //  WITH HOOK, LATIN CAPITAL LETTER
    ('\u{0197}', 'I'), //  WITH STROKE, LATIN CAPITAL LETTER
    ('\u{0199}', 'k'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{019A}', 'l'), //  WITH BAR, LATIN SMALL LETTER
    ('\u{019C}', 'M'), // , LATIN CAPITAL LETTER TURNED
    ('\u{019D}', 'N'), //  WITH LEFT HOOK, LATIN CAPITAL LETTER
    ('\u{019E}', 'n'), //  WITH LONG RIGHT LEG, LATIN SMALL LETTER
    ('\u{019F}', 'O'), //  WITH MIDDLE TILDE, LATIN CAPITAL LETTER
    ('\u{01A1}', 'o'), //  WITH HORN, LATIN SMALL LETTER
    ('\u{01A5}', 'p'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{01AB}', 't'), //  WITH PALATAL HOOK, LATIN SMALL LETTER
    ('\u{01AD}', 't'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{01AE}', 'T'), //  WITH RETROFLEX HOOK, LATIN CAPITAL LETTER
    ('\u{01B0}', 'u'), //  WITH HORN, LATIN SMALL LETTER
    ('\u{01B2}', 'V'), //  WITH HOOK, LATIN CAPITAL LETTER
    ('\u{01B4}', 'y'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{01B6}', 'z'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{01CE}', 'a'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{01D0}', 'i'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{01D2}', 'o'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{01D4}', 'u'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{01DD}', 'e'), // , LATIN SMALL LETTER TURNED
    ('\u{01E5}', 'g'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{01E7}', 'g'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{01E9}', 'k'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{01EB}', 'o'), //  WITH OGONEK, LATIN SMALL LETTER
    ('\u{01F0}', 'j'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{01F5}', 'g'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{01F9}', 'n'), //  WITH GRAVE, LATIN SMALL LETTER
    ('\u{0201}', 'a'), //  WITH DOUBLE GRAVE, LATIN SMALL LETTER
    ('\u{0203}', 'a'), //  WITH INVERTED BREVE, LATIN SMALL LETTER
    ('\u{0205}', 'e'), //  WITH DOUBLE GRAVE, LATIN SMALL LETTER
    ('\u{0207}', 'e'), //  WITH INVERTED BREVE, LATIN SMALL LETTER
    ('\u{0209}', 'i'), //  WITH DOUBLE GRAVE, LATIN SMALL LETTER
    ('\u{020B}', 'i'), //  WITH INVERTED BREVE, LATIN SMALL LETTER
    ('\u{020D}', 'o'), //  WITH DOUBLE GRAVE, LATIN SMALL LETTER
    ('\u{020F}', 'o'), //  WITH INVERTED BREVE, LATIN SMALL LETTER
    ('\u{0211}', 'r'), //  WITH DOUBLE GRAVE, LATIN SMALL LETTER
    ('\u{0213}', 'r'), //  WITH INVERTED BREVE, LATIN SMALL LETTER
    ('\u{0215}', 'u'), //  WITH DOUBLE GRAVE, LATIN SMALL LETTER
    ('\u{0217}', 'u'), //  WITH INVERTED BREVE, LATIN SMALL LETTER
    ('\u{0219}', 's'), //  WITH COMMA BELOW, LATIN SMALL LETTER
    ('\u{021B}', 't'), //  WITH COMMA BELOW, LATIN SMALL LETTER
    ('\u{021F}', 'h'), //  WITH CARON, LATIN SMALL LETTER
    ('\u{0220}', 'N'), //  WITH LONG RIGHT LEG, LATIN CAPITAL LETTER
    ('\u{0221}', 'd'), //  WITH CURL, LATIN SMALL LETTER
    ('\u{0225}', 'z'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{0227}', 'a'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{0229}', 'e'), //  WITH CEDILLA, LATIN SMALL LETTER
    ('\u{022F}', 'o'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{0233}', 'y'), //  WITH MACRON, LATIN SMALL LETTER
    ('\u{0234}', 'l'), //  WITH CURL, LATIN SMALL LETTER
    ('\u{0235}', 'n'), //  WITH CURL, LATIN SMALL LETTER
    ('\u{0236}', 't'), //  WITH CURL, LATIN SMALL LETTER
    ('\u{0237}', 'j'), // , LATIN SMALL LETTER DOTLESS
    ('\u{023A}', 'A'), //  WITH STROKE, LATIN CAPITAL LETTER
    ('\u{023B}', 'C'), //  WITH STROKE, LATIN CAPITAL LETTER
    ('\u{023C}', 'c'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{023D}', 'L'), //  WITH BAR, LATIN CAPITAL LETTER
    ('\u{023E}', 'T'), //  WITH DIAGONAL STROKE, LATIN CAPITAL LETTER
    ('\u{023F}', 's'), //  WITH SWASH TAIL, LATIN SMALL LETTER
    ('\u{0240}', 'z'), //  WITH SWASH TAIL, LATIN SMALL LETTER
    ('\u{0243}', 'B'), //  WITH STROKE, LATIN CAPITAL LETTER
    ('\u{0244}', 'U'), //  BAR, LATIN CAPITAL LETTER
    ('\u{0245}', 'V'), // , LATIN CAPITAL LETTER TURNED
    ('\u{0246}', 'E'), //  WITH STROKE, LATIN CAPITAL LETTER
    ('\u{0247}', 'e'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{0248}', 'J'), //  WITH STROKE, LATIN CAPITAL LETTER
    ('\u{0249}', 'j'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{024A}', 'Q'), //  WITH HOOK TAIL, LATIN CAPITAL LETTER SMALL
    ('\u{024B}', 'q'), //  WITH HOOK TAIL, LATIN SMALL LETTER
    ('\u{024C}', 'R'), //  WITH STROKE, LATIN CAPITAL LETTER
    ('\u{024D}', 'r'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{024E}', 'Y'), //  WITH STROKE, LATIN CAPITAL LETTER
    ('\u{024F}', 'y'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{0250}', 'a'), // , LATIN SMALL LETTER TURNED
    ('\u{0251}', 'a'), // , latin small letter script
    ('\u{0253}', 'b'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{0254}', 'o'), // , LATIN SMALL LETTER OPEN
    ('\u{0255}', 'c'), //  WITH CURL, LATIN SMALL LETTER
    ('\u{0256}', 'd'), //  WITH TAIL, LATIN SMALL LETTER
    ('\u{0257}', 'd'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{0258}', 'e'), // , LATIN SMALL LETTER REVERSED
    ('\u{025B}', 'e'), // , LATIN SMALL LETTER OPEN
    ('\u{025C}', 'e'), // , LATIN SMALL LETTER REVERSED OPEN
    ('\u{025D}', 'e'), //  WITH HOOK, LATIN SMALL LETTER REVERSED OPEN
    ('\u{025E}', 'e'), // , LATIN SMALL LETTER CLOSED REVERSED OPEN
    ('\u{025F}', 'j'), //  WITH STROKE, LATIN SMALL LETTER DOTLESS
    ('\u{0260}', 'g'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{0261}', 'g'), // , LATIN SMALL LETTER SCRIPT
    ('\u{0262}', 'G'), // , LATIN LETTER SMALL CAPITAL
    ('\u{0265}', 'h'), // , LATIN SMALL LETTER TURNED
    ('\u{0266}', 'h'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{0268}', 'i'), //  WITH STROKE, LATIN SMALL LETTER
    ('\u{026A}', 'I'), // , LATIN LETTER SMALL CAPITAL
    ('\u{026B}', 'l'), //  WITH MIDDLE TILDE, LATIN SMALL LETTER
    ('\u{026C}', 'l'), //  WITH BELT, LATIN SMALL LETTER
    ('\u{026D}', 'l'), //  WITH RETROFLEX HOOK, LATIN SMALL LETTER
    ('\u{026F}', 'm'), // , LATIN SMALL LETTER TURNED
    ('\u{0270}', 'm'), //  WITH LONG LEG, LATIN SMALL LETTER TURNED
    ('\u{0271}', 'm'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{0272}', 'n'), //  WITH LEFT HOOK, LATIN SMALL LETTER
    ('\u{0273}', 'n'), //  WITH RETROFLEX HOOK, LATIN SMALL LETTER
    ('\u{0274}', 'N'), // , LATIN LETTER SMALL CAPITAL
    ('\u{0275}', 'o'), // , LATIN SMALL LETTER BARRED
    ('\u{0279}', 'r'), // , LATIN SMALL LETTER TURNED
    ('\u{027A}', 'r'), //  WITH LONG LEG, LATIN SMALL LETTER TURNED
    ('\u{027B}', 'r'), //  WITH HOOK, LATIN SMALL LETTER TURNED
    ('\u{027C}', 'r'), //  WITH LONG LEG, LATIN SMALL LETTER
    ('\u{027D}', 'r'), //  WITH TAIL, LATIN SMALL LETTER
    ('\u{027E}', 'r'), //  WITH FISHHOOK, LATIN SMALL LETTER
    ('\u{027F}', 'r'), //  WITH FISHHOOK, LATIN SMALL LETTER REVERSED
    ('\u{0280}', 'R'), // , LATIN LETTER SMALL CAPITAL
    ('\u{0281}', 'R'), // , LATIN LETTER SMALL CAPITAL INVERTED
    ('\u{0282}', 's'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{0287}', 't'), // , LATIN SMALL LETTER TURNED
    ('\u{0288}', 't'), //  WITH RETROFLEX HOOK, LATIN SMALL LETTER
    ('\u{0289}', 'u'), //  BAR, LATIN SMALL LETTER
    ('\u{028B}', 'v'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{028C}', 'v'), // , LATIN SMALL LETTER TURNED
    ('\u{028D}', 'w'), // , LATIN SMALL LETTER TURNED
    ('\u{028E}', 'y'), // , LATIN SMALL LETTER TURNED
    ('\u{028F}', 'Y'), // , LATIN LETTER SMALL CAPITAL
    ('\u{0290}', 'z'), //  WITH RETROFLEX HOOK, LATIN SMALL LETTER
    ('\u{0291}', 'z'), //  WITH CURL, LATIN SMALL LETTER
    ('\u{0297}', 'c'), // , LATIN LETTER STRETCHED
    ('\u{0299}', 'B'), // , LATIN LETTER SMALL CAPITAL
    ('\u{029A}', 'e'), // , LATIN SMALL LETTER CLOSED OPEN
    ('\u{029B}', 'G'), //  WITH HOOK, LATIN LETTER SMALL CAPITAL
    ('\u{029C}', 'H'), // , LATIN LETTER SMALL CAPITAL
    ('\u{029D}', 'j'), //  WITH CROSSED-TAIL, LATIN SMALL LETTER
    ('\u{029E}', 'k'), // , LATIN SMALL LETTER TURNED
    ('\u{029F}', 'L'), // , LATIN LETTER SMALL CAPITAL
    ('\u{02A0}', 'q'), //  WITH HOOK, LATIN SMALL LETTER
    ('\u{02AE}', 'h'), //  WITH FISHHOOK, LATIN SMALL LETTER TURNED
    ('\u{0363}', 'a'), // , COMBINING LATIN SMALL LETTER
    ('\u{0364}', 'e'), // , COMBINING LATIN SMALL LETTER
    ('\u{0365}', 'i'), // , COMBINING LATIN SMALL LETTER
    ('\u{0366}', 'o'), // , COMBINING LATIN SMALL LETTER
    ('\u{0367}', 'u'), // , COMBINING LATIN SMALL LETTER
    ('\u{0368}', 'c'), // , COMBINING LATIN SMALL LETTER
    ('\u{0369}', 'd'), // , COMBINING LATIN SMALL LETTER
    ('\u{036A}', 'h'), // , COMBINING LATIN SMALL LETTER
    ('\u{036B}', 'm'), // , COMBINING LATIN SMALL LETTER
    ('\u{036C}', 'r'), // , COMBINING LATIN SMALL LETTER
    ('\u{036D}', 't'), // , COMBINING LATIN SMALL LETTER
    ('\u{036E}', 'v'), // , COMBINING LATIN SMALL LETTER
    ('\u{036F}', 'x'), // , COMBINING LATIN SMALL LETTER
];

const DATA2: [(char, char); 167] = [
    ('\u{1D00}', 'A'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D03}', 'B'), // , LATIN LETTER SMALL CAPITAL BARRED
    ('\u{1D04}', 'C'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D05}', 'D'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D07}', 'E'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D08}', 'e'), // , LATIN SMALL LETTER TURNED OPEN
    ('\u{1D09}', 'i'), // , LATIN SMALL LETTER TURNED
    ('\u{1D0A}', 'J'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D0B}', 'K'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D0C}', 'L'), //  WITH STROKE, LATIN LETTER SMALL CAPITAL
    ('\u{1D0D}', 'M'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D0E}', 'N'), // , LATIN LETTER SMALL CAPITAL REVERSED
    ('\u{1D0F}', 'O'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D10}', 'O'), // , LATIN LETTER SMALL CAPITAL OPEN
    ('\u{1D11}', 'o'), // , LATIN SMALL LETTER SIDEWAYS
    ('\u{1D12}', 'o'), // , LATIN SMALL LETTER SIDEWAYS OPEN
    ('\u{1D13}', 'o'), //  WITH STROKE, LATIN SMALL LETTER SIDEWAYS
    ('\u{1D16}', 'o'), // , LATIN SMALL LETTER TOP HALF
    ('\u{1D17}', 'o'), // , LATIN SMALL LETTER BOTTOM HALF
    ('\u{1D18}', 'P'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D19}', 'R'), // , LATIN LETTER SMALL CAPITAL REVERSED
    ('\u{1D1A}', 'R'), // , LATIN LETTER SMALL CAPITAL TURNED
    ('\u{1D1B}', 'T'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D1C}', 'U'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D1D}', 'u'), // , LATIN SMALL LETTER SIDEWAYS
    ('\u{1D1E}', 'u'), // , LATIN SMALL LETTER SIDEWAYS DIAERESIZED
    ('\u{1D1F}', 'm'), // , LATIN SMALL LETTER SIDEWAYS TURNED
    ('\u{1D20}', 'V'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D21}', 'W'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D22}', 'Z'), // , LATIN LETTER SMALL CAPITAL
    ('\u{1D62}', 'i'), // , LATIN SUBSCRIPT SMALL LETTER
    ('\u{1D63}', 'r'), // , LATIN SUBSCRIPT SMALL LETTER
    ('\u{1D64}', 'u'), // , LATIN SUBSCRIPT SMALL LETTER
    ('\u{1D65}', 'v'), // , LATIN SUBSCRIPT SMALL LETTER
    ('\u{1E01}', 'a'), //  WITH RING BELOW, LATIN SMALL LETTER
    ('\u{1E03}', 'b'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E05}', 'b'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E07}', 'b'), //  WITH LINE BELOW, LATIN SMALL LETTER
    ('\u{1E0B}', 'd'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E0D}', 'd'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E0F}', 'd'), //  WITH LINE BELOW, LATIN SMALL LETTER
    ('\u{1E11}', 'd'), //  WITH CEDILLA, LATIN SMALL LETTER
    ('\u{1E13}', 'd'), //  WITH CIRCUMFLEX BELOW, LATIN SMALL LETTER
    ('\u{1E19}', 'e'), //  WITH CIRCUMFLEX BELOW, LATIN SMALL LETTER
    ('\u{1E1B}', 'e'), //  WITH TILDE BELOW, LATIN SMALL LETTER
    ('\u{1E1F}', 'f'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E21}', 'g'), //  WITH MACRON, LATIN SMALL LETTER
    ('\u{1E23}', 'h'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E25}', 'h'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E27}', 'h'), //  WITH DIAERESIS, LATIN SMALL LETTER
    ('\u{1E29}', 'h'), //  WITH CEDILLA, LATIN SMALL LETTER
    ('\u{1E2B}', 'h'), //  WITH BREVE BELOW, LATIN SMALL LETTER
    ('\u{1E2D}', 'i'), //  WITH TILDE BELOW, LATIN SMALL LETTER
    ('\u{1E31}', 'k'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{1E33}', 'k'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E35}', 'k'), //  WITH LINE BELOW, LATIN SMALL LETTER
    ('\u{1E37}', 'l'), //  WITH DOT BELOW, LATIN SMALL LETTER    ('\u{1E3B}', 'l'), //  WITH LINE BELOW, LATIN SMALL LETTER
    ('\u{1E3D}', 'l'), //  WITH CIRCUMFLEX BELOW, LATIN SMALL LETTER
    ('\u{1E3F}', 'm'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{1E41}', 'm'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E43}', 'm'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E45}', 'n'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E47}', 'n'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E49}', 'n'), //  WITH LINE BELOW, LATIN SMALL LETTER
    ('\u{1E4B}', 'n'), //  WITH CIRCUMFLEX BELOW, LATIN SMALL LETTER
    ('\u{1E55}', 'p'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{1E57}', 'p'), //  WITH DOT ABOVE, LATIN SMALL LETTER    ('\u{1E59}', 'r'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E5B}', 'r'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E5F}', 'r'), //  WITH LINE BELOW, LATIN SMALL LETTER
    ('\u{1E61}', 's'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E63}', 's'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E6B}', 't'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E6D}', 't'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E6F}', 't'), //  WITH LINE BELOW, LATIN SMALL LETTER
    ('\u{1E71}', 't'), //  WITH CIRCUMFLEX BELOW, LATIN SMALL LETTER
    ('\u{1E73}', 'u'), //  WITH DIAERESIS BELOW, LATIN SMALL LETTER
    ('\u{1E75}', 'u'), //  WITH TILDE BELOW, LATIN SMALL LETTER    ('\u{1E77}', 'u'), //  WITH CIRCUMFLEX BELOW, LATIN SMALL LETTER
    ('\u{1E7D}', 'v'), //  WITH TILDE, LATIN SMALL LETTER
    ('\u{1E7F}', 'v'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E81}', 'w'), //  WITH GRAVE, LATIN SMALL LETTER
    ('\u{1E83}', 'w'), //  WITH ACUTE, LATIN SMALL LETTER
    ('\u{1E85}', 'w'), //  WITH DIAERESIS, LATIN SMALL LETTER    ('\u{1E87}', 'w'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E89}', 'w'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E8B}', 'x'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E8D}', 'x'), //  WITH DIAERESIS, LATIN SMALL LETTER
    ('\u{1E8F}', 'y'), //  WITH DOT ABOVE, LATIN SMALL LETTER
    ('\u{1E91}', 'z'), //  WITH CIRCUMFLEX, LATIN SMALL LETTER    ('\u{1E93}', 'z'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1E95}', 'z'), //  WITH LINE BELOW, LATIN SMALL LETTER
    ('\u{1E96}', 'h'), //  WITH LINE BELOW, LATIN SMALL LETTER
    ('\u{1E97}', 't'), //  WITH DIAERESIS, LATIN SMALL LETTER
    ('\u{1E98}', 'w'), //  WITH RING ABOVE, LATIN SMALL LETTER
    ('\u{1E99}', 'y'), //  WITH RING ABOVE, LATIN SMALL LETTER
    ('\u{1E9A}', 'a'), //  WITH RIGHT HALF RING, LATIN SMALL LETTER
    ('\u{1E9B}', 's'), //  WITH DOT ABOVE, LATIN SMALL LETTER LONG
    ('\u{1EA1}', 'a'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1EA3}', 'a'), //  WITH HOOK ABOVE, LATIN SMALL LETTER
    ('\u{1EB9}', 'e'), //  WITH DOT BELOW, LATIN SMALL LETTER    ('\u{1EBB}', 'e'), //  WITH HOOK ABOVE, LATIN SMALL LETTER
    ('\u{1EBD}', 'e'), //  WITH TILDE, LATIN SMALL LETTER
    ('\u{1EC9}', 'i'), //  WITH HOOK ABOVE, LATIN SMALL LETTER
    ('\u{1ECB}', 'i'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1ECD}', 'o'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1ECF}', 'o'), //  WITH HOOK ABOVE, LATIN SMALL LETTER
    ('\u{1EE5}', 'u'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1EE7}', 'u'), //  WITH HOOK ABOVE, LATIN SMALL LETTER
    ('\u{1EF3}', 'y'), //  WITH GRAVE, LATIN SMALL LETTER
    ('\u{1EF5}', 'y'), //  WITH DOT BELOW, LATIN SMALL LETTER
    ('\u{1EF7}', 'y'), //  WITH HOOK ABOVE, LATIN SMALL LETTER    ('\u{1EF9}', 'y'), //  WITH TILDE, LATIN SMALL LETTER
    ('\u{1ea4}', 'A'),
    ('\u{1ea5}', 'a'),
    ('\u{1ea6}', 'A'),
    ('\u{1ea7}', 'a'),
    ('\u{1ea8}', 'A'),
    ('\u{1ea9}', 'a'),
    ('\u{1eaa}', 'A'),
    ('\u{1eab}', 'a'),
    ('\u{1eac}', 'A'),
    ('\u{1ead}', 'a'),
    ('\u{1eae}', 'A'),
    ('\u{1eaf}', 'a'),
    ('\u{1eb0}', 'A'),
    ('\u{1eb1}', 'a'),
    ('\u{1eb2}', 'A'),
    ('\u{1eb3}', 'a'),
    ('\u{1eb4}', 'A'),
    ('\u{1eb5}', 'a'),
    ('\u{1eb6}', 'A'),
    ('\u{1eb7}', 'a'),
    ('\u{1ebe}', 'E'),
    ('\u{1ebf}', 'e'),
    ('\u{1ec0}', 'E'),
    ('\u{1ec1}', 'e'),
    ('\u{1ec2}', 'E'),
    ('\u{1ec3}', 'e'),
    ('\u{1ec4}', 'E'),
    ('\u{1ec5}', 'e'),
    ('\u{1ec6}', 'E'),
    ('\u{1ec7}', 'e'),
    ('\u{1ed0}', 'O'),
    ('\u{1ed1}', 'o'),
    ('\u{1ed2}', 'O'),
    ('\u{1ed3}', 'o'),
    ('\u{1ed4}', 'O'),
    ('\u{1ed5}', 'o'),
    ('\u{1ed6}', 'O'),
    ('\u{1ed7}', 'o'),
    ('\u{1ed8}', 'O'),
    ('\u{1ed9}', 'o'),
    ('\u{1eda}', 'O'),
    ('\u{1edb}', 'o'),
    ('\u{1edc}', 'O'),
    ('\u{1edd}', 'o'),
    ('\u{1ede}', 'O'),
    ('\u{1edf}', 'o'),
    ('\u{1ee0}', 'O'),
    ('\u{1ee1}', 'o'),
    ('\u{1ee2}', 'O'),
    ('\u{1ee3}', 'o'),
    ('\u{1ee8}', 'U'),
    ('\u{1ee9}', 'u'),
    ('\u{1eea}', 'U'),
    ('\u{1eeb}', 'u'),
    ('\u{1eec}', 'U'),
    ('\u{1eed}', 'u'),
    ('\u{1eee}', 'U'),
    ('\u{1eef}', 'u'),
    ('\u{1ef0}', 'U'),
    ('\u{1ef1}', 'u'),
];

const DATA3: [(char, char); 9] = [
    ('\u{2071}', 'i'), // , SUPERSCRIPT LATIN SMALL LETTER
    ('\u{2095}', 'h'), // , LATIN SUBSCRIPT SMALL LETTER
    ('\u{2096}', 'k'), // , LATIN SUBSCRIPT SMALL LETTER
    ('\u{2097}', 'l'), // , LATIN SUBSCRIPT SMALL LETTER
    ('\u{2098}', 'm'), // , LATIN SUBSCRIPT SMALL LETTER0x2099: 'n', // , LATIN SUBSCRIPT SMALL LETTER
    ('\u{209A}', 'p'), // , LATIN SUBSCRIPT SMALL LETTER
    ('\u{209B}', 's'), // , LATIN SUBSCRIPT SMALL LETTER
    ('\u{209C}', 't'), // , LATIN SUBSCRIPT SMALL LETTER
    ('\u{2184}', 'c'), // , LATIN SMALL LETTER REVERSED
];

const DATA1_START: u32 = DATA1[0].0 as u32;
const DATA1_END: u32 = DATA1[DATA1.len() - 1].0 as u32 + 1;
const LEN1: usize = (DATA1_END - DATA1_START) as usize;
static TABLE1: [char; LEN1] = generate_table(&DATA1);

const fn generate_table<const LEN: usize>(sparse_data: &[(char, char)]) -> [char; LEN] {
    let mut table: [char; LEN] = ['\0'; LEN];
    let start = sparse_data[0].0 as u32;
    let mut i = 0u32;
    let mut j = 0;
    while i < table.len() as u32 {
        let key = unsafe { transmute(start + i) };
        if sparse_data[j].0 == key {
            table[i as usize] = DATA1[j].1;
            j += 1;
        } else {
            //identity
            table[i as usize] = key;
        }
        i += 1;
    }
    table
}
const DATA2_START: u32 = DATA2[0].0 as u32;
const DATA2_END: u32 = DATA2[DATA2.len() - 1].0 as u32 + 1;
const LEN2: usize = (DATA2_END - DATA2_START) as usize;
static TABLE2: [char; LEN2] = generate_table(&DATA2);

const DATA3_START: u32 = DATA3[0].0 as u32;
const DATA3_END: u32 = DATA3[DATA3.len() - 1].0 as u32 + 1;
const LEN3: usize = (DATA3_END - DATA3_START) as usize;
static TABLE3: [char; LEN3] = generate_table(&DATA3);

/// Normalizes a unicode character by converting latin characters
/// which are variants of ASCII characters to their latin equivant.
///
/// # Example
///
/// ``` rust
/// # use nucleo_matcher::chars::normalize;
///
/// assert_eq!(normalize('ä'), 'a');
/// ```
pub fn normalize(c: char) -> char {
    let i = c as u32;
    if i < DATA1_START || i >= DATA3_END {
        return c;
    }
    if i < DATA1_END {
        return TABLE1[(i - DATA1_START) as usize];
    }
    if i < DATA2_START {
        return c;
    }
    if i < DATA2_END {
        return TABLE2[(i - DATA2_START) as usize];
    }
    if i < DATA3_START {
        return c;
    }
    TABLE3[(i - DATA3_START) as usize]
}
