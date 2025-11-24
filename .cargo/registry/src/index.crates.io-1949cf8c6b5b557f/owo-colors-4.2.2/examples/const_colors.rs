//! Example demonstrating colors in const contexts.

use owo_colors::{colors::*, styles::*, *};

const GREEN_TEXT: FgColorDisplay<Green, str> = FgColorDisplay::new("green text");
const RED_BG_TEXT: BgColorDisplay<Red, str> = BgColorDisplay::new("red background");
const COMBO_TEXT: ComboColorDisplay<Blue, White, str> =
    ComboColorDisplay::new("blue text on white background");
const DYN_RED_TEXT: FgDynColorDisplay<AnsiColors, str> =
    FgDynColorDisplay::new("red text (dynamic)", AnsiColors::Red);
const DYN_GREEN_BG_TEXT: BgDynColorDisplay<AnsiColors, str> =
    BgDynColorDisplay::new("green background (dynamic)", AnsiColors::Green);
const COMBO_DYN_TEXT: ComboDynColorDisplay<XtermColors, XtermColors, str> =
    ComboDynColorDisplay::new(
        "blue text on lilac background (dynamic)",
        XtermColors::BlueRibbon,
        XtermColors::WistfulLilac,
    );

const BOLD_TEXT: BoldDisplay<str> = BoldDisplay("bold text");
const DIM_TEXT: DimDisplay<str> = DimDisplay("dim text");
const ITALIC_TEXT: ItalicDisplay<str> = ItalicDisplay("italic text");
const UNDERLINE_TEXT: UnderlineDisplay<str> = UnderlineDisplay("underlined text");
const BLINK_TEXT: BlinkDisplay<str> = BlinkDisplay("blinking text");
const BLINK_FAST_TEXT: BlinkFastDisplay<str> = BlinkFastDisplay("fast blinking text");
const REVERSED_TEXT: ReversedDisplay<str> = ReversedDisplay("reversed text");
const HIDDEN_TEXT: HiddenDisplay<str> = HiddenDisplay("hidden text");
const STRIKETHROUGH_TEXT: StrikeThroughDisplay<str> = StrikeThroughDisplay("strikethrough text");

const STYLED_TEXT: Styled<&'static str> = Style::new()
    .bold()
    .italic()
    .red()
    .style("bold and italic red text (dynamically styled)");
const STYLED_TEXT_2: Styled<&'static str> = Style::new()
    .effect(Effect::Underline)
    .effects(&[Effect::Dimmed, Effect::Strikethrough])
    .green()
    .style("underlined, dimmed and strikethrough green text (dynamically styled)");

fn main() {
    println!("{}", GREEN_TEXT);
    println!("{}", RED_BG_TEXT);
    println!("{}", COMBO_TEXT);
    println!("{}", DYN_RED_TEXT);
    println!("{}", DYN_GREEN_BG_TEXT);
    println!("{}", COMBO_DYN_TEXT);

    println!("{}", BOLD_TEXT);
    println!("{}", DIM_TEXT);
    println!("{}", ITALIC_TEXT);
    println!("{}", UNDERLINE_TEXT);
    println!("{}", BLINK_TEXT);
    println!("{}", BLINK_FAST_TEXT);
    println!("{}", REVERSED_TEXT);
    println!("{}", HIDDEN_TEXT);
    println!("{}", STRIKETHROUGH_TEXT);

    println!("{}", STYLED_TEXT);
    println!("{}", STYLED_TEXT_2);
}
