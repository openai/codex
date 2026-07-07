/// Modifier keys held while forwarding input to the browser.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrowserInputModifiers {
    /// Alt/Option is held.
    pub alt: bool,
    /// Control is held.
    pub control: bool,
    /// Command/Super is held.
    pub meta: bool,
    /// Shift is held.
    pub shift: bool,
}

impl BrowserInputModifiers {
    pub(crate) fn cdp_mask(self) -> u8 {
        u8::from(self.alt)
            | (u8::from(self.control) << 1)
            | (u8::from(self.meta) << 2)
            | (u8::from(self.shift) << 3)
    }
}

/// One keyboard event forwarded while the user exclusively controls the browser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserKeyInput {
    /// DOM key value, such as `Enter` or `a`.
    pub key: String,
    /// DOM physical-key code, such as `Enter` or `KeyA`.
    pub code: String,
    /// Text to insert for printable keys; absent for shortcuts and control keys.
    pub text: Option<String>,
    /// Modifier state accompanying the key.
    pub modifiers: BrowserInputModifiers,
}

/// Mouse button used by a forwarded terminal mouse event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BrowserMouseButton {
    #[default]
    None,
    Left,
    Middle,
    Right,
}

/// Kind of mouse event forwarded during exclusive human control.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BrowserMouseKind {
    /// Pointer movement without a button transition.
    Move,
    /// Mouse-button press.
    Down,
    /// Mouse-button release.
    Up,
    /// Scroll-wheel movement in CSS pixels.
    Wheel { delta_x: f64, delta_y: f64 },
}

/// Terminal-relative mouse event forwarded to Carbonyl's native input parser.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrowserMouseInput {
    /// Mouse operation to perform.
    pub kind: BrowserMouseKind,
    /// Button associated with the event.
    pub button: BrowserMouseButton,
    /// Zero-based terminal column inside the rendered browser viewport.
    pub column: u16,
    /// Zero-based terminal row inside the rendered browser viewport.
    pub row: u16,
    /// Width of the rendered browser viewport in terminal cells.
    pub viewport_cols: u16,
    /// Height of the rendered browser viewport in terminal cells.
    pub viewport_rows: u16,
    /// Modifier state accompanying the mouse event.
    pub modifiers: BrowserInputModifiers,
}
