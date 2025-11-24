use yansi::{Paint, Style, Condition, Color::*};

static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

macro_rules! assert_renders {
    ($($input:expr => $expected:expr,)*) => {
        let _lock = LOCK.lock().expect("FAIL FAST - LOCK POISONED");
        yansi::enable();

        $(
            let (input, expected) = ($input.to_string(), $expected.to_string());
            if input != expected {
                panic!("\nexpected: {:?} {}\nactual:   {:?} {}\n\
                    global, condition = {}, {}\n\
                    input = {}\nstyle = {:#?}\n",
                    expected, expected, input, input,
                    yansi::is_enabled(), $input.style.enabled(),
                    stringify!($input), $input.style);
            }
        )*

        yansi::whenever(Condition::DEFAULT);
    };
}

macro_rules! assert_disabled_renders {
    ($($input:expr => $expected:expr,)*) => {
        let _lock = LOCK.lock().expect("FAIL FAST - LOCK POISONED");
        $(
            yansi::disable();
            let (actual, expected) = ($input.to_string(), $expected.to_string());
            yansi::enable();
            assert_eq!(actual, expected);

            let local = $input.whenever(yansi::Condition::NEVER);
            let (actual, expected) = (local.to_string(), $expected.to_string());
            assert_eq!(actual, expected);
        )*
    };
}

#[test]
fn colors_enabled() {
    assert_renders! {
        Paint::new("text/plain") => "text/plain",
        Paint::red("hi") => "\x1B[31mhi\x1B[0m",
        Paint::black("hi") => "\x1B[30mhi\x1B[0m",
        Paint::yellow("hi").bold() => "\x1B[1;33mhi\x1B[0m",
        Paint::new("hi").fg(Yellow).bold() => "\x1B[1;33mhi\x1B[0m",
        Paint::blue("hi").underline() => "\x1B[4;34mhi\x1B[0m",
        Paint::green("hi").bold().underline() => "\x1B[1;4;32mhi\x1B[0m",
        Paint::green("hi").underline().bold() => "\x1B[1;4;32mhi\x1B[0m",
        Paint::magenta("hi").bg(White) => "\x1B[47;35mhi\x1B[0m",
        Paint::red("hi").bg(Blue).fg(Yellow) => "\x1B[44;33mhi\x1B[0m",
        Paint::cyan("hi").bg(Blue).fg(Yellow) => "\x1B[44;33mhi\x1B[0m",
        Paint::cyan("hi").bold().bg(White) => "\x1B[1;47;36mhi\x1B[0m",
        Paint::cyan("hi").underline().bg(White) => "\x1B[4;47;36mhi\x1B[0m",
        Paint::cyan("hi").bold().underline().bg(White) => "\x1B[1;4;47;36mhi\x1B[0m",
        Paint::cyan("hi").underline().bold().bg(White) => "\x1B[1;4;47;36mhi\x1B[0m",
        Paint::new("hi").fixed(100) => "\x1B[38;5;100mhi\x1B[0m",
        Paint::new("hi").fixed(100).bg(Magenta) => "\x1B[45;38;5;100mhi\x1B[0m",
        Paint::new("hi").fixed(100).bg(Fixed(200)) => "\x1B[48;5;200;38;5;100mhi\x1B[0m",
        Paint::new("hi").rgb(70, 130, 180) => "\x1B[38;2;70;130;180mhi\x1B[0m",
        Paint::new("hi").rgb(70, 130, 180).bg(Blue) => "\x1B[44;38;2;70;130;180mhi\x1B[0m",
        Paint::blue("hi").bg(Rgb(70, 130, 180)) => "\x1B[48;2;70;130;180;34mhi\x1B[0m",
        Paint::new("hi").rgb(70, 130, 180).bg(Rgb(5,10,15)) =>
            "\x1B[48;2;5;10;15;38;2;70;130;180mhi\x1B[0m",
        Paint::new("hi").bold() => "\x1B[1mhi\x1B[0m",
        Paint::new("hi").underline() => "\x1B[4mhi\x1B[0m",
        Paint::new("hi").bold().underline() => "\x1B[1;4mhi\x1B[0m",
        Paint::new("hi").dim() => "\x1B[2mhi\x1B[0m",
        Paint::new("hi").italic() => "\x1B[3mhi\x1B[0m",
        Paint::new("hi").blink() => "\x1B[5mhi\x1B[0m",
        Paint::new("hi").invert() => "\x1B[7mhi\x1B[0m",
        Paint::new("hi").conceal() => "\x1B[8mhi\x1B[0m",
        Paint::new("hi").strike() => "\x1B[9mhi\x1B[0m",
    }
}

#[test]
fn colors_disabled() {
    assert_disabled_renders! {
        Paint::new("text/plain") => "text/plain",
        Paint::red("hi") => "hi",
        Paint::black("hi") => "hi",
        Paint::yellow("hi").bold() => "hi",
        Paint::new("hi").fg(Yellow).bold() => "hi",
        Paint::blue("hi").underline() => "hi",
        Paint::green("hi").bold().underline() => "hi",
        Paint::green("hi").underline().bold() => "hi",
        Paint::magenta("hi").bg(White) => "hi",
        Paint::red("hi").bg(Blue).fg(Yellow) => "hi",
        Paint::cyan("hi").bg(Blue).fg(Yellow) => "hi",
        Paint::cyan("hi").bold().bg(White) => "hi",
        Paint::cyan("hi").underline().bg(White) => "hi",
        Paint::cyan("hi").bold().underline().bg(White) => "hi",
        Paint::cyan("hi").underline().bold().bg(White) => "hi",
        Paint::new("hi").fixed(100) => "hi",
        Paint::new("hi").fixed(100).bg(Magenta) => "hi",
        Paint::new("hi").fixed(100).bg(Fixed(200)) => "hi",
        Paint::new("hi").rgb(70, 130, 180) => "hi",
        Paint::new("hi").rgb(70, 130, 180).bg(Blue) => "hi",
        Paint::blue("hi").bg(Rgb(70, 130, 180)) => "hi",
        Paint::blue("hi").bg(Rgb(70, 130, 180)).wrap() => "hi",
        Paint::new("hi").rgb(70, 130, 180).bg(Rgb(5,10,15)) => "hi",
        Paint::new("hi").bold() => "hi",
        Paint::new("hi").underline() => "hi",
        Paint::new("hi").bold().underline() => "hi",
        Paint::new("hi").dim() => "hi",
        Paint::new("hi").italic() => "hi",
        Paint::new("hi").blink() => "hi",
        Paint::new("hi").invert() => "hi",
        Paint::new("hi").conceal() => "hi",
        Paint::new("hi").strike() => "hi",
        Paint::new("hi").strike().wrap() => "hi",
    }
}

#[test]
fn masked_when_disabled() {
    assert_disabled_renders! {
        Paint::mask("text/plain") => "",
        Paint::mask("text/plain").mask() => "",
        Paint::new("text/plain").mask() => "",
        Paint::new("text/plain").mask() => "",
        Paint::red("hi").mask() => "",
        Paint::black("hi").mask() => "",
        Paint::yellow("hi").bold().mask() => "",
        Paint::cyan("hi").bg(Blue).fg(Yellow).mask() => "",
        Paint::cyan("hi").underline().bold().bg(White).mask() => "",
    }
}

#[test]
fn masked_when_enabled() {
    assert_renders! {
        Paint::mask("text/plain") => "text/plain",
        Paint::mask("text/plain").mask() => "text/plain",
        Paint::black("hi").mask() => "\x1B[30mhi\x1B[0m",
        Paint::yellow("hi").bold().mask() => "\x1B[1;33mhi\x1B[0m",
        Paint::new("hi").fg(Yellow).bold().mask() => "\x1B[1;33mhi\x1B[0m",
        Paint::cyan("hi").underline().bg(White).mask() => "\x1B[4;47;36mhi\x1B[0m",
        Paint::cyan("hi").bold().underline().bg(White).mask() => "\x1B[1;4;47;36mhi\x1B[0m",
        Paint::new("hi").rgb(70, 130, 180).mask() => "\x1B[38;2;70;130;180mhi\x1B[0m",
        Paint::new("hi").underline().mask() => "\x1B[4mhi\x1B[0m",
        Paint::new("hi").bold().underline().mask() => "\x1B[1;4mhi\x1B[0m",
        Paint::new("hi").conceal().mask() => "\x1B[8mhi\x1B[0m",
    }
}

#[test]
#[cfg(feature = "alloc")]
fn wrapping() {
    let inner = || format!("{} b {}", Paint::red("a"), Paint::green("c"));
    let inner2 = || format!("0 {} 1", Paint::magenta(&inner()).wrap());
    assert_renders! {
        Paint::new("text/plain").wrap() => "text/plain",
        Paint::new(&inner()).wrap() => &inner(),
        Paint::new(&inner()).wrap() =>
            "\u{1b}[31ma\u{1b}[0m b \u{1b}[32mc\u{1b}[0m",
        Paint::new(&inner()).fg(Blue).wrap() =>
            "\u{1b}[34m\u{1b}[31ma\u{1b}[0m\u{1b}[34m b \
            \u{1b}[32mc\u{1b}[0m\u{1b}[34m\u{1b}[0m",
        Paint::new(&inner2()).wrap() => &inner2(),
        Paint::new(&inner2()).wrap() =>
            "0 \u{1b}[35m\u{1b}[31ma\u{1b}[0m\u{1b}[35m b \
            \u{1b}[32mc\u{1b}[0m\u{1b}[35m\u{1b}[0m 1",
        Paint::new(&inner2()).fg(Blue).wrap() =>
            "\u{1b}[34m0 \u{1b}[35m\u{1b}[31ma\u{1b}[0m\u{1b}[34m\u{1b}[35m b \
            \u{1b}[32mc\u{1b}[0m\u{1b}[34m\u{1b}[35m\u{1b}[0m\u{1b}[34m 1\u{1b}[0m",
    }
}

#[test]
fn lingering() {
    let _lock = LOCK.lock().expect("FAIL FAST - LOCK POISONED");
    yansi::enable();

    assert_eq! {
        format!("Hello! {} {} things with {} {}?",
            "How".magenta().underline().linger(),
            "are".italic(),
            "you".on_yellow().linger(),
            "today".blue()),
        "Hello! \u{1b}[4;35mHow \u{1b}[3mare\u{1b}[0m things with \
            \u{1b}[43myou \u{1b}[34mtoday\u{1b}[0m?",
    };

    assert_eq! {
        format!("Hi! {} {} things with {} {}?",
            "How".magenta().underline().linger(),
            "are".italic().linger(),
            "you".on_yellow().linger(),
            "today".blue()),
        "Hi! \u{1b}[4;35mHow \u{1b}[3mare things with \u{1b}[43myou \
            \u{1b}[34mtoday\u{1b}[0m?"
    };

    assert_eq! {
        format!("{} B {} {} {} F",
            "A".red().linger(),
            "C".underline().linger(),
            "D", // doesn't linger, but no styling applied, thus no reset
            "E".resetting()),  // explicitly reset
        "\u{1b}[31mA B \u{1b}[4mC D E\u{1b}[0m F"
    };

    assert_eq! {
        format!("{} B {} {} {} F",
            "A".red().linger(),
            "C".underline().linger(),
            "D", // doesn't linger, but no styling applied, thus no reset
            "E".clear()),  // explicitly reset
        "\u{1b}[31mA B \u{1b}[4mC D E\u{1b}[0m F"
    };

    yansi::whenever(Condition::DEFAULT);
}

#[test]
fn hash_eq() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash<T: Hash>(t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    let a = Style::default();
    let b = Style::default().mask();
    let c = Style::new();

    assert_eq!(a, b);
    assert_eq!(b, c);
    assert_eq!(hash(&a), hash(&b));
    assert_eq!(hash(&a), hash(&c));
}
