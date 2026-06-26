use vt100::Callbacks;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            rows: 30,
            cols: 100,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserStatus {
    Unavailable { reason: String },
    Idle,
    Starting,
    Running,
    Crashed { message: String },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BrowserColor {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrowserCell {
    pub text: String,
    pub foreground: BrowserColor,
    pub background: BrowserColor,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underlined: bool,
    pub reversed: bool,
    pub wide_continuation: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserScreen {
    pub rows: u16,
    pub cols: u16,
    pub cells: Vec<BrowserCell>,
    pub cursor: Option<(u16, u16)>,
}

impl BrowserScreen {
    pub fn blank(size: TerminalSize) -> Self {
        let len = usize::from(size.rows) * usize::from(size.cols);
        Self {
            rows: size.rows,
            cols: size.cols,
            cells: vec![BrowserCell::default(); len],
            cursor: None,
        }
    }

    pub fn cell(&self, row: u16, col: u16) -> Option<&BrowserCell> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let index = usize::from(row) * usize::from(self.cols) + usize::from(col);
        self.cells.get(index)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserView {
    pub status: BrowserStatus,
    pub title: Option<String>,
    pub url: Option<String>,
    pub visible: bool,
    pub screen: BrowserScreen,
}

impl BrowserView {
    pub(crate) fn new(status: BrowserStatus, size: TerminalSize) -> Self {
        Self {
            status,
            title: None,
            url: None,
            visible: false,
            screen: BrowserScreen::blank(size),
        }
    }
}

#[derive(Default)]
struct ScreenCallbacks {
    title: Option<String>,
}

impl Callbacks for ScreenCallbacks {
    fn set_window_title(&mut self, _screen: &mut vt100::Screen, title: &[u8]) {
        let title = String::from_utf8_lossy(title);
        self.title = Some(title.chars().take(/*n*/ 512).collect());
    }
}

pub(crate) struct TerminalScreen {
    parser: vt100::Parser<ScreenCallbacks>,
    query_responder: TerminalQueryResponder,
}

impl TerminalScreen {
    pub(crate) fn new(size: TerminalSize) -> Self {
        Self {
            parser: vt100::Parser::new_with_callbacks(
                size.rows,
                size.cols,
                /*scrollback_len*/ 0,
                ScreenCallbacks::default(),
            ),
            query_responder: TerminalQueryResponder::default(),
        }
    }

    pub(crate) fn process(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.parser.process(bytes);
        self.query_responder.process(bytes)
    }

    pub(crate) fn resize(&mut self, size: TerminalSize) {
        self.parser.screen_mut().set_size(size.rows, size.cols);
    }

    pub(crate) fn title(&self) -> Option<String> {
        self.parser.callbacks().title.clone()
    }

    pub(crate) fn snapshot(&self) -> BrowserScreen {
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        let mut cells = Vec::with_capacity(usize::from(rows) * usize::from(cols));
        for row in 0..rows {
            for col in 0..cols {
                cells.push(screen.cell(row, col).map(cell_from_vt).unwrap_or_default());
            }
        }
        let cursor = (!screen.hide_cursor()).then(|| screen.cursor_position());
        BrowserScreen {
            rows,
            cols,
            cells,
            cursor,
        }
    }
}

fn cell_from_vt(cell: &vt100::Cell) -> BrowserCell {
    BrowserCell {
        text: cell.contents().to_string(),
        foreground: color_from_vt(cell.fgcolor()),
        background: color_from_vt(cell.bgcolor()),
        bold: cell.bold(),
        dim: cell.dim(),
        italic: cell.italic(),
        underlined: cell.underline(),
        reversed: cell.inverse(),
        wide_continuation: cell.is_wide_continuation(),
    }
}

fn color_from_vt(color: vt100::Color) -> BrowserColor {
    match color {
        vt100::Color::Default => BrowserColor::Default,
        vt100::Color::Idx(index) => BrowserColor::Indexed(index),
        vt100::Color::Rgb(red, green, blue) => BrowserColor::Rgb(red, green, blue),
    }
}

const TRUE_COLOR_QUERY: &[u8] = b"\x1bP$qm\x1b\\";
const TRUE_COLOR_RESPONSE: &[u8] = b"\x1bP1$r48:2:0:0:0m\x1b\\";
const TERMINAL_NAME_QUERY: &[u8] = b"\x1bP+q544e\x1b\\";
const TERMINAL_NAME_RESPONSE: &[u8] = b"\x1bP1+r544e=787465726d2d323536636f6c6f72\x1b\\";
const QUERY_TAIL_LEN: usize = 32;

#[derive(Default)]
pub(crate) struct TerminalQueryResponder {
    pending: Vec<u8>,
}

impl TerminalQueryResponder {
    pub(crate) fn process(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.pending.extend_from_slice(bytes);
        let mut responses = Vec::new();
        drain_query(
            &mut self.pending,
            TRUE_COLOR_QUERY,
            TRUE_COLOR_RESPONSE,
            &mut responses,
        );
        drain_query(
            &mut self.pending,
            TERMINAL_NAME_QUERY,
            TERMINAL_NAME_RESPONSE,
            &mut responses,
        );
        if self.pending.len() > QUERY_TAIL_LEN {
            let start = self.pending.len() - QUERY_TAIL_LEN;
            self.pending.drain(..start);
        }
        responses
    }
}

fn drain_query(pending: &mut Vec<u8>, query: &[u8], response: &[u8], responses: &mut Vec<Vec<u8>>) {
    while let Some(start) = pending
        .windows(query.len())
        .position(|window| window == query)
    {
        let end = start + query.len();
        pending.drain(start..end);
        responses.push(response.to_vec());
    }
}
