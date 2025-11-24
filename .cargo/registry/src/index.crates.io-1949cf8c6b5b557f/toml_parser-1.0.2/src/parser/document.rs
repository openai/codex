use winnow::stream::Offset as _;
use winnow::stream::Stream as _;
use winnow::stream::TokenSlice;

use super::EventReceiver;
#[cfg(feature = "debug")]
use crate::debug::DebugErrorSink;
#[cfg(feature = "debug")]
use crate::debug::DebugEventReceiver;
use crate::decoder::Encoding;
use crate::lexer::Token;
use crate::lexer::TokenKind;
use crate::ErrorSink;
use crate::Expected;
use crate::ParseError;

/// Parse lexed tokens into [`Event`][super::Event]s
pub fn parse_document(
    tokens: &[Token],
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    let mut tokens = TokenSlice::new(tokens);
    #[cfg(feature = "debug")]
    let mut receiver = DebugEventReceiver::new(receiver);
    #[cfg(feature = "debug")]
    let receiver = &mut receiver;
    #[cfg(feature = "debug")]
    let mut error = DebugErrorSink::new(error);
    #[cfg(feature = "debug")]
    let error = &mut error;
    document(&mut tokens, receiver, error);
    eof(&mut tokens, receiver, error);
}

/// Parse lexed tokens into [`Event`][super::Event]s
pub fn parse_key(tokens: &[Token], receiver: &mut dyn EventReceiver, error: &mut dyn ErrorSink) {
    let mut tokens = TokenSlice::new(tokens);
    #[cfg(feature = "debug")]
    let mut receiver = DebugEventReceiver::new(receiver);
    #[cfg(feature = "debug")]
    let receiver = &mut receiver;
    #[cfg(feature = "debug")]
    let mut error = DebugErrorSink::new(error);
    #[cfg(feature = "debug")]
    let error = &mut error;
    key(&mut tokens, "invalid key", receiver, error);
    eof(&mut tokens, receiver, error);
}

/// Parse lexed tokens into [`Event`][super::Event]s
pub fn parse_simple_key(
    tokens: &[Token],
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    let mut tokens = TokenSlice::new(tokens);
    #[cfg(feature = "debug")]
    let mut receiver = DebugEventReceiver::new(receiver);
    #[cfg(feature = "debug")]
    let receiver = &mut receiver;
    #[cfg(feature = "debug")]
    let mut error = DebugErrorSink::new(error);
    #[cfg(feature = "debug")]
    let error = &mut error;
    simple_key(&mut tokens, "invalid key", receiver, error);
    eof(&mut tokens, receiver, error);
}

/// Parse lexed tokens into [`Event`][super::Event]s
pub fn parse_value(tokens: &[Token], receiver: &mut dyn EventReceiver, error: &mut dyn ErrorSink) {
    let mut tokens = TokenSlice::new(tokens);
    #[cfg(feature = "debug")]
    let mut receiver = DebugEventReceiver::new(receiver);
    #[cfg(feature = "debug")]
    let receiver = &mut receiver;
    #[cfg(feature = "debug")]
    let mut error = DebugErrorSink::new(error);
    #[cfg(feature = "debug")]
    let error = &mut error;
    value(&mut tokens, receiver, error);
    eof(&mut tokens, receiver, error);
}

type Stream<'i> = TokenSlice<'i, Token>;

/// Parse a TOML Document
///
/// Only the order of [`Event`][super::Event]s is validated and not [`Event`][super::Event] content nor semantics like duplicate
/// keys.
///
/// ```bnf
/// toml = expression *( newline expression )
///
/// expression =  ws [ comment ]
/// expression =/ ws keyval ws [ comment ]
/// expression =/ ws table ws [ comment ]
///
/// ;; Key-Value pairs
///
/// keyval = key keyval-sep val
///
/// key = simple-key / dotted-key
/// simple-key = quoted-key / unquoted-key
///
/// quoted-key = basic-string / literal-string
/// dotted-key = simple-key 1*( dot-sep simple-key )
///
/// dot-sep   = ws %x2E ws  ; . Period
/// keyval-sep = ws %x3D ws ; =
///
/// val = string / boolean / array / inline-table / date-time / float / integer
///
/// ;; Array
///
/// array = array-open [ array-values ] ws-comment-newline array-close
///
/// array-open =  %x5B ; [
/// array-close = %x5D ; ]
///
/// array-values =  ws-comment-newline val ws-comment-newline array-sep array-values
/// array-values =/ ws-comment-newline val ws-comment-newline [ array-sep ]
///
/// array-sep = %x2C  ; , Comma
///
/// ;; Table
///
/// table = std-table / array-table
///
/// ;; Standard Table
///
/// std-table = std-table-open key std-table-close
///
/// ;; Inline Table
///
/// inline-table = inline-table-open [ inline-table-keyvals ] inline-table-close
///
/// inline-table-keyvals = keyval [ inline-table-sep inline-table-keyvals ]
///
/// ;; Array Table
///
/// array-table = array-table-open key array-table-close
/// ```
fn document(tokens: &mut Stream<'_>, receiver: &mut dyn EventReceiver, error: &mut dyn ErrorSink) {
    while let Some(current_token) = tokens.next_token() {
        match current_token.kind() {
            TokenKind::LeftSquareBracket => on_table(tokens, current_token, receiver, error),
            TokenKind::RightSquareBracket => {
                on_missing_std_table(tokens, current_token, receiver, error);
            }
            TokenKind::LiteralString => on_expression_key(
                tokens,
                current_token,
                Some(Encoding::LiteralString),
                receiver,
                error,
            ),
            TokenKind::BasicString => on_expression_key(
                tokens,
                current_token,
                Some(Encoding::BasicString),
                receiver,
                error,
            ),
            TokenKind::MlLiteralString => on_expression_key(
                tokens,
                current_token,
                Some(Encoding::MlLiteralString),
                receiver,
                error,
            ),
            TokenKind::MlBasicString => on_expression_key(
                tokens,
                current_token,
                Some(Encoding::MlBasicString),
                receiver,
                error,
            ),
            TokenKind::Atom => on_expression_key(tokens, current_token, None, receiver, error),
            TokenKind::Equals => {
                let fake_key = current_token.span().before();
                let encoding = None;
                receiver.simple_key(fake_key, encoding, error);
                on_expression_key_val_sep(tokens, current_token, receiver, error);
            }
            TokenKind::Dot => {
                on_expression_dot(tokens, current_token, receiver, error);
            }
            TokenKind::Comma | TokenKind::RightCurlyBracket | TokenKind::LeftCurlyBracket => {
                on_missing_expression_key(tokens, current_token, receiver, error);
            }
            TokenKind::Whitespace => receiver.whitespace(current_token.span(), error),
            TokenKind::Newline => receiver.newline(current_token.span(), error),
            TokenKind::Comment => on_comment(tokens, current_token, receiver, error),
            TokenKind::Eof => {
                break;
            }
        }
    }
}

/// Start a table from the open token
///
/// This eats to EOL
///
/// ```bnf
/// ;; Table
///
/// table = std-table / array-table
///
/// ;; Standard Table
///
/// std-table = std-table-open key std-table-close
///
/// ;; Array Table
///
/// array-table = array-table-open key array-table-close
/// ```
fn on_table(
    tokens: &mut Stream<'_>,
    open_token: &Token,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    let is_array_table = if let Some(second_open_token) =
        next_token_if(tokens, |k| matches!(k, TokenKind::LeftSquareBracket))
    {
        let span = open_token.span().append(second_open_token.span());
        receiver.array_table_open(span, error);
        true
    } else {
        let span = open_token.span();
        receiver.std_table_open(span, error);
        false
    };

    opt_whitespace(tokens, receiver, error);

    let valid_key = key(tokens, "invalid table", receiver, error);

    opt_whitespace(tokens, receiver, error);

    let mut success = false;
    if let Some(close_token) = next_token_if(tokens, |k| matches!(k, TokenKind::RightSquareBracket))
    {
        if is_array_table {
            if let Some(second_close_token) =
                next_token_if(tokens, |k| matches!(k, TokenKind::RightSquareBracket))
            {
                let span = close_token.span().append(second_close_token.span());
                receiver.array_table_close(span, error);
                success = true;
            } else {
                let context = open_token.span().append(close_token.span());
                error.report_error(
                    ParseError::new("unclosed array table")
                        .with_context(context)
                        .with_expected(&[Expected::Literal("]")])
                        .with_unexpected(close_token.span().after()),
                );
            }
        } else {
            receiver.std_table_close(close_token.span(), error);
            success = true;
        }
    } else if valid_key {
        let last_key_token = tokens
            .previous_tokens()
            .find(|t| t.kind() != TokenKind::Whitespace)
            .unwrap_or(open_token);
        let context = open_token.span().append(last_key_token.span());
        if is_array_table {
            error.report_error(
                ParseError::new("unclosed array table")
                    .with_context(context)
                    .with_expected(&[Expected::Literal("]]")])
                    .with_unexpected(last_key_token.span().after()),
            );
        } else {
            error.report_error(
                ParseError::new("unclosed table")
                    .with_context(context)
                    .with_expected(&[Expected::Literal("]")])
                    .with_unexpected(last_key_token.span().after()),
            );
        }
    }

    if success {
        ws_comment_newline(tokens, receiver, error);
    } else {
        ignore_to_newline(tokens, receiver, error);
    }
}

/// Parse a TOML key
///
/// ```bnf
/// ;; Key-Value pairs
///
/// key = simple-key / dotted-key
/// simple-key = quoted-key / unquoted-key
///
/// quoted-key = basic-string / literal-string
/// dotted-key = simple-key 1*( dot-sep simple-key )
///
/// dot-sep   = ws %x2E ws  ; . Period
/// ```
fn key(
    tokens: &mut Stream<'_>,
    invalid_description: &'static str,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) -> bool {
    while let Some(current_token) = tokens.next_token() {
        let encoding = match current_token.kind() {
            TokenKind::RightSquareBracket
            | TokenKind::Comment
            | TokenKind::Equals
            | TokenKind::Comma
            | TokenKind::LeftSquareBracket
            | TokenKind::LeftCurlyBracket
            | TokenKind::RightCurlyBracket
            | TokenKind::Newline
            | TokenKind::Eof => {
                let fake_key = current_token.span().before();
                let encoding = None;
                receiver.simple_key(fake_key, encoding, error);
                seek(tokens, -1);
                return false;
            }
            TokenKind::Whitespace => {
                receiver.whitespace(current_token.span(), error);
                continue;
            }
            TokenKind::Dot => {
                let fake_key = current_token.span().before();
                let encoding = None;
                receiver.simple_key(fake_key, encoding, error);
                receiver.key_sep(current_token.span(), error);
                continue;
            }
            TokenKind::LiteralString => Some(Encoding::LiteralString),
            TokenKind::BasicString => Some(Encoding::BasicString),
            TokenKind::MlLiteralString => Some(Encoding::MlLiteralString),
            TokenKind::MlBasicString => Some(Encoding::MlBasicString),
            TokenKind::Atom => None,
        };
        receiver.simple_key(current_token.span(), encoding, error);
        return opt_dot_keys(tokens, receiver, error);
    }

    let previous_span = tokens
        .previous_tokens()
        .find(|t| {
            !matches!(
                t.kind(),
                TokenKind::Whitespace | TokenKind::Comment | TokenKind::Newline | TokenKind::Eof
            )
        })
        .map(|t| t.span())
        .unwrap_or_default();
    error.report_error(
        ParseError::new(invalid_description)
            .with_context(previous_span)
            .with_expected(&[Expected::Description("key")])
            .with_unexpected(previous_span.after()),
    );
    false
}

/// Start an expression from a key compatible token  type
///
/// ```abnf
/// expression =  ws [ comment ]
/// expression =/ ws keyval ws [ comment ]
/// expression =/ ws table ws [ comment ]
///
/// ;; Key-Value pairs
///
/// keyval = key keyval-sep val
/// ```
fn on_expression_key<'i>(
    tokens: &mut Stream<'i>,
    key_token: &'i Token,
    encoding: Option<Encoding>,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    receiver.simple_key(key_token.span(), encoding, error);
    opt_dot_keys(tokens, receiver, error);

    opt_whitespace(tokens, receiver, error);

    let Some(eq_token) = next_token_if(tokens, |k| matches!(k, TokenKind::Equals)) else {
        if let Some(peek_token) = tokens.first() {
            let span = peek_token.span().before();
            error.report_error(
                ParseError::new("key with no value")
                    .with_context(span)
                    .with_expected(&[Expected::Literal("=")])
                    .with_unexpected(span),
            );
        }
        ignore_to_newline(tokens, receiver, error);
        return;
    };
    on_expression_key_val_sep(tokens, eq_token, receiver, error);
}

fn on_expression_dot<'i>(
    tokens: &mut Stream<'i>,
    dot_token: &'i Token,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    receiver.simple_key(dot_token.span().before(), None, error);
    seek(tokens, -1);
    opt_dot_keys(tokens, receiver, error);

    opt_whitespace(tokens, receiver, error);

    let Some(eq_token) = next_token_if(tokens, |k| matches!(k, TokenKind::Equals)) else {
        if let Some(peek_token) = tokens.first() {
            let span = peek_token.span().before();
            error.report_error(
                ParseError::new("missing value for key")
                    .with_context(span)
                    .with_expected(&[Expected::Literal("=")])
                    .with_unexpected(span),
            );
        }
        ignore_to_newline(tokens, receiver, error);
        return;
    };
    on_expression_key_val_sep(tokens, eq_token, receiver, error);
}

fn on_expression_key_val_sep<'i>(
    tokens: &mut Stream<'i>,
    eq_token: &'i Token,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    receiver.key_val_sep(eq_token.span(), error);

    opt_whitespace(tokens, receiver, error);

    value(tokens, receiver, error);

    ws_comment_newline(tokens, receiver, error);
}

/// Parse a TOML simple key
///
/// ```bnf
/// ;; Key-Value pairs
///
/// simple-key = quoted-key / unquoted-key
///
/// quoted-key = basic-string / literal-string
/// ```
fn simple_key(
    tokens: &mut Stream<'_>,
    invalid_description: &'static str,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    let Some(current_token) = tokens.next_token() else {
        let previous_span = tokens
            .previous_tokens()
            .find(|t| {
                !matches!(
                    t.kind(),
                    TokenKind::Whitespace
                        | TokenKind::Comment
                        | TokenKind::Newline
                        | TokenKind::Eof
                )
            })
            .map(|t| t.span())
            .unwrap_or_default();
        error.report_error(
            ParseError::new(invalid_description)
                .with_context(previous_span)
                .with_expected(&[Expected::Description("key")])
                .with_unexpected(previous_span.after()),
        );
        return;
    };

    const EXPECTED_KEYS: [Expected; 3] = [
        Expected::Description(Encoding::LiteralString.description()),
        Expected::Description(Encoding::BasicString.description()),
        Expected::Description(UNQUOTED_STRING),
    ];

    let kind = match current_token.kind() {
        TokenKind::Dot
        | TokenKind::RightSquareBracket
        | TokenKind::Comment
        | TokenKind::Equals
        | TokenKind::Comma
        | TokenKind::LeftSquareBracket
        | TokenKind::LeftCurlyBracket
        | TokenKind::RightCurlyBracket
        | TokenKind::Newline
        | TokenKind::Eof
        | TokenKind::Whitespace => {
            on_missing_key(tokens, current_token, invalid_description, receiver, error);
            return;
        }
        TokenKind::LiteralString => Some(Encoding::LiteralString),
        TokenKind::BasicString => Some(Encoding::BasicString),
        TokenKind::MlLiteralString => {
            error.report_error(
                ParseError::new(invalid_description)
                    .with_context(current_token.span())
                    .with_expected(&EXPECTED_KEYS)
                    .with_unexpected(current_token.span()),
            );
            Some(Encoding::MlLiteralString)
        }
        TokenKind::MlBasicString => {
            error.report_error(
                ParseError::new(invalid_description)
                    .with_context(current_token.span())
                    .with_expected(&EXPECTED_KEYS)
                    .with_unexpected(current_token.span()),
            );
            Some(Encoding::MlBasicString)
        }
        TokenKind::Atom => None,
    };
    receiver.simple_key(current_token.span(), kind, error);
}

/// Start a key from the first key compatible token type
///
/// Returns the last key on success
///
/// This will swallow the trailing [`TokenKind::Whitespace`]
///
/// ```abnf
/// key = simple-key / dotted-key
/// simple-key = quoted-key / unquoted-key
///
/// quoted-key = basic-string / literal-string
/// dotted-key = simple-key 1*( dot-sep simple-key )
///
/// dot-sep   = ws %x2E ws  ; . Period
/// ```
fn opt_dot_keys(
    tokens: &mut Stream<'_>,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) -> bool {
    opt_whitespace(tokens, receiver, error);

    let mut success = true;
    'dot: while let Some(dot_token) = next_token_if(tokens, |k| matches!(k, TokenKind::Dot)) {
        receiver.key_sep(dot_token.span(), error);

        while let Some(current_token) = tokens.next_token() {
            let kind = match current_token.kind() {
                TokenKind::Equals
                | TokenKind::Comma
                | TokenKind::LeftSquareBracket
                | TokenKind::RightSquareBracket
                | TokenKind::LeftCurlyBracket
                | TokenKind::RightCurlyBracket
                | TokenKind::Comment
                | TokenKind::Newline
                | TokenKind::Eof => {
                    let fake_key = current_token.span().before();
                    let encoding = None;
                    receiver.simple_key(fake_key, encoding, error);
                    seek(tokens, -1);

                    success = false;
                    break 'dot;
                }
                TokenKind::Whitespace => {
                    receiver.whitespace(current_token.span(), error);
                    continue;
                }
                TokenKind::Dot => {
                    let fake_key = current_token.span().before();
                    let encoding = None;
                    receiver.simple_key(fake_key, encoding, error);
                    receiver.key_sep(current_token.span(), error);
                    continue;
                }
                TokenKind::LiteralString => Some(Encoding::LiteralString),
                TokenKind::BasicString => Some(Encoding::BasicString),
                TokenKind::MlLiteralString => Some(Encoding::MlLiteralString),
                TokenKind::MlBasicString => Some(Encoding::MlBasicString),
                TokenKind::Atom => None,
            };
            receiver.simple_key(current_token.span(), kind, error);
            opt_whitespace(tokens, receiver, error);
            continue 'dot;
        }

        let fake_key = dot_token.span().after();
        let encoding = None;
        receiver.simple_key(fake_key, encoding, error);
    }

    success
}

/// Parse a value
///
/// ```abnf
/// val = string / boolean / array / inline-table / date-time / float / integer
/// ```
fn value(tokens: &mut Stream<'_>, receiver: &mut dyn EventReceiver, error: &mut dyn ErrorSink) {
    let Some(current_token) = tokens.next_token() else {
        let previous_span = tokens
            .previous_tokens()
            .find(|t| {
                !matches!(
                    t.kind(),
                    TokenKind::Whitespace
                        | TokenKind::Comment
                        | TokenKind::Newline
                        | TokenKind::Eof
                )
            })
            .map(|t| t.span())
            .unwrap_or_default();
        error.report_error(
            ParseError::new("missing value")
                .with_context(previous_span)
                .with_expected(&[Expected::Description("value")])
                .with_unexpected(previous_span.after()),
        );
        return;
    };

    match current_token.kind() {
        TokenKind::Comment
        | TokenKind::Comma
        | TokenKind::Newline
        | TokenKind::Eof
        | TokenKind::Whitespace => {
            let fake_key = current_token.span().before();
            let encoding = None;
            receiver.scalar(fake_key, encoding, error);
            seek(tokens, -1);
        }
        TokenKind::Equals => {
            error.report_error(
                ParseError::new("extra `=`")
                    .with_context(current_token.span())
                    .with_expected(&[])
                    .with_unexpected(current_token.span()),
            );
            receiver.error(current_token.span(), error);
            value(tokens, receiver, error);
        }
        TokenKind::LeftCurlyBracket => {
            on_inline_table_open(tokens, current_token, receiver, error);
        }
        TokenKind::RightCurlyBracket => {
            error.report_error(
                ParseError::new("missing inline table opening")
                    .with_context(current_token.span())
                    .with_expected(&[Expected::Literal("{")])
                    .with_unexpected(current_token.span().before()),
            );

            let _ = receiver.inline_table_open(current_token.span().before(), error);
            receiver.inline_table_close(current_token.span(), error);
        }
        TokenKind::LeftSquareBracket => {
            on_array_open(tokens, current_token, receiver, error);
        }
        TokenKind::RightSquareBracket => {
            error.report_error(
                ParseError::new("missing array opening")
                    .with_context(current_token.span())
                    .with_expected(&[Expected::Literal("[")])
                    .with_unexpected(current_token.span().before()),
            );

            let _ = receiver.array_open(current_token.span().before(), error);
            receiver.array_close(current_token.span(), error);
        }
        TokenKind::LiteralString
        | TokenKind::BasicString
        | TokenKind::MlLiteralString
        | TokenKind::MlBasicString
        | TokenKind::Dot
        | TokenKind::Atom => {
            on_scalar(tokens, current_token, receiver, error);
        }
    }
}

/// Parse a scalar value
///
/// ```abnf
/// val = string / boolean / array / inline-table / date-time / float / integer
/// ```
fn on_scalar(
    tokens: &mut Stream<'_>,
    scalar: &Token,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    let mut span = scalar.span();
    let encoding = match scalar.kind() {
        TokenKind::Comment
        | TokenKind::Comma
        | TokenKind::Newline
        | TokenKind::Eof
        | TokenKind::Whitespace
        | TokenKind::Equals
        | TokenKind::LeftCurlyBracket
        | TokenKind::RightCurlyBracket
        | TokenKind::LeftSquareBracket
        | TokenKind::RightSquareBracket => {
            unreachable!()
        }
        TokenKind::LiteralString => Some(Encoding::LiteralString),
        TokenKind::BasicString => Some(Encoding::BasicString),
        TokenKind::MlLiteralString => Some(Encoding::MlLiteralString),
        TokenKind::MlBasicString => Some(Encoding::MlBasicString),
        TokenKind::Dot | TokenKind::Atom => {
            while let Some(next_token) = tokens.first() {
                match next_token.kind() {
                    TokenKind::Comment
                    | TokenKind::Comma
                    | TokenKind::Newline
                    | TokenKind::Eof
                    | TokenKind::Equals
                    | TokenKind::LeftCurlyBracket
                    | TokenKind::RightCurlyBracket
                    | TokenKind::LeftSquareBracket
                    | TokenKind::RightSquareBracket
                    | TokenKind::LiteralString
                    | TokenKind::BasicString
                    | TokenKind::MlLiteralString
                    | TokenKind::MlBasicString => {
                        break;
                    }
                    TokenKind::Whitespace => {
                        if let Some(second) = tokens.get(1) {
                            if second.kind() == TokenKind::Atom {
                                span = span.append(second.span());
                                let _ = tokens.next_slice(2);
                                continue;
                            }
                        }
                        break;
                    }
                    TokenKind::Dot | TokenKind::Atom => {
                        span = span.append(next_token.span());
                        let _ = tokens.next_token();
                    }
                }
            }
            None
        }
    };
    receiver.scalar(span, encoding, error);
}

/// Parse an array
///
/// ```abnf
/// ;; Array
///
/// array = array-open [ array-values ] ws-comment-newline array-close
///
/// array-values =  ws-comment-newline val ws-comment-newline array-sep array-values
/// array-values =/ ws-comment-newline val ws-comment-newline [ array-sep ]
/// ```
fn on_array_open(
    tokens: &mut Stream<'_>,
    array_open: &Token,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    if !receiver.array_open(array_open.span(), error) {
        ignore_to_value_close(tokens, TokenKind::RightSquareBracket, receiver, error);
        return;
    }

    enum State {
        NeedsValue,
        NeedsComma,
    }

    let mut state = State::NeedsValue;
    while let Some(current_token) = tokens.next_token() {
        match current_token.kind() {
            TokenKind::Comment => {
                on_comment(tokens, current_token, receiver, error);
            }
            TokenKind::Whitespace => {
                receiver.whitespace(current_token.span(), error);
            }
            TokenKind::Newline => {
                receiver.newline(current_token.span(), error);
            }
            TokenKind::Eof => {
                error.report_error(
                    ParseError::new("unclosed array")
                        .with_context(array_open.span())
                        .with_expected(&[Expected::Literal("]")])
                        .with_unexpected(current_token.span()),
                );
                receiver.array_close(current_token.span().before(), error);
                return;
            }
            TokenKind::Comma => match state {
                State::NeedsValue => {
                    error.report_error(
                        ParseError::new("extra comma in array")
                            .with_context(array_open.span())
                            .with_expected(&[Expected::Description("value")])
                            .with_unexpected(current_token.span()),
                    );
                    receiver.error(current_token.span(), error);
                }
                State::NeedsComma => {
                    receiver.value_sep(current_token.span(), error);

                    state = State::NeedsValue;
                }
            },
            TokenKind::Equals => {
                error.report_error(
                    ParseError::new("unexpected `=` in array")
                        .with_context(array_open.span())
                        .with_expected(&[Expected::Description("value"), Expected::Literal("]")])
                        .with_unexpected(current_token.span()),
                );
                receiver.error(current_token.span(), error);
            }
            TokenKind::LeftCurlyBracket => {
                if !matches!(state, State::NeedsValue) {
                    error.report_error(
                        ParseError::new("missing comma between array elements")
                            .with_context(array_open.span())
                            .with_expected(&[Expected::Literal(",")])
                            .with_unexpected(current_token.span().before()),
                    );
                    receiver.value_sep(current_token.span().before(), error);
                }

                on_inline_table_open(tokens, current_token, receiver, error);

                state = State::NeedsComma;
            }
            TokenKind::RightCurlyBracket => {
                if !matches!(state, State::NeedsValue) {
                    error.report_error(
                        ParseError::new("missing comma between array elements")
                            .with_context(array_open.span())
                            .with_expected(&[Expected::Literal(",")])
                            .with_unexpected(current_token.span().before()),
                    );
                    receiver.value_sep(current_token.span().before(), error);
                }

                error.report_error(
                    ParseError::new("missing inline table opening")
                        .with_context(current_token.span())
                        .with_expected(&[Expected::Literal("{")])
                        .with_unexpected(current_token.span().before()),
                );

                let _ = receiver.inline_table_open(current_token.span().before(), error);
                receiver.inline_table_close(current_token.span(), error);

                state = State::NeedsComma;
            }
            TokenKind::LeftSquareBracket => {
                if !matches!(state, State::NeedsValue) {
                    error.report_error(
                        ParseError::new("missing comma between array elements")
                            .with_context(array_open.span())
                            .with_expected(&[Expected::Literal(",")])
                            .with_unexpected(current_token.span().before()),
                    );
                    receiver.value_sep(current_token.span().before(), error);
                }

                on_array_open(tokens, current_token, receiver, error);

                state = State::NeedsComma;
            }
            TokenKind::RightSquareBracket => {
                receiver.array_close(current_token.span(), error);

                return;
            }
            TokenKind::LiteralString
            | TokenKind::BasicString
            | TokenKind::MlLiteralString
            | TokenKind::MlBasicString
            | TokenKind::Dot
            | TokenKind::Atom => {
                if !matches!(state, State::NeedsValue) {
                    error.report_error(
                        ParseError::new("missing comma between array elements")
                            .with_context(array_open.span())
                            .with_expected(&[Expected::Literal(",")])
                            .with_unexpected(current_token.span().before()),
                    );
                    receiver.value_sep(current_token.span().before(), error);
                }

                on_scalar(tokens, current_token, receiver, error);

                state = State::NeedsComma;
            }
        }
    }

    let previous_span = tokens
        .previous_tokens()
        .find(|t| {
            !matches!(
                t.kind(),
                TokenKind::Whitespace | TokenKind::Comment | TokenKind::Newline | TokenKind::Eof
            )
        })
        .map(|t| t.span())
        .unwrap_or_default();
    error.report_error(
        ParseError::new("unclosed array")
            .with_context(array_open.span())
            .with_expected(&[Expected::Literal("]")])
            .with_unexpected(previous_span.after()),
    );
    receiver.array_close(previous_span.after(), error);
}

/// Parse an inline table
///
/// ```abnf
/// ;; Inline Table
///
/// inline-table = inline-table-open [ inline-table-keyvals ] inline-table-close
///
/// inline-table-keyvals = keyval [ inline-table-sep inline-table-keyvals ]
/// ```
fn on_inline_table_open(
    tokens: &mut Stream<'_>,
    inline_table_open: &Token,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    if !receiver.inline_table_open(inline_table_open.span(), error) {
        ignore_to_value_close(tokens, TokenKind::RightCurlyBracket, receiver, error);
        return;
    }

    #[allow(clippy::enum_variant_names)]
    #[derive(Debug)]
    enum State {
        NeedsKey,
        NeedsEquals,
        NeedsValue,
        NeedsComma,
    }

    impl State {
        fn expected(&self) -> &'static [Expected] {
            match self {
                Self::NeedsKey => &[Expected::Description("key")],
                Self::NeedsEquals => &[Expected::Literal("=")],
                Self::NeedsValue => &[Expected::Description("value")],
                Self::NeedsComma => &[Expected::Literal(",")],
            }
        }
    }

    let mut empty = true;
    let mut state = State::NeedsKey;
    while let Some(current_token) = tokens.next_token() {
        match current_token.kind() {
            TokenKind::Comment => {
                error.report_error(
                    ParseError::new("comments are unsupported in inline tables")
                        .with_context(inline_table_open.span())
                        .with_expected(&[])
                        .with_unexpected(current_token.span()),
                );

                on_comment(tokens, current_token, receiver, error);
            }
            TokenKind::Whitespace => {
                receiver.whitespace(current_token.span(), error);
            }
            TokenKind::Newline => {
                error.report_error(
                    ParseError::new("newlines are unsupported in inline tables")
                        .with_context(inline_table_open.span())
                        .with_expected(&[])
                        .with_unexpected(current_token.span()),
                );

                receiver.newline(current_token.span(), error);
            }
            TokenKind::Eof => {
                error.report_error(
                    ParseError::new("unclosed inline table")
                        .with_context(inline_table_open.span())
                        .with_expected(&[Expected::Literal("}")])
                        .with_unexpected(current_token.span()),
                );

                receiver.inline_table_close(current_token.span().before(), error);
                return;
            }
            TokenKind::Comma => match state {
                State::NeedsKey | State::NeedsEquals | State::NeedsValue => {
                    error.report_error(
                        ParseError::new("extra comma in inline table")
                            .with_context(inline_table_open.span())
                            .with_expected(state.expected())
                            .with_unexpected(current_token.span().before()),
                    );
                    receiver.error(current_token.span(), error);
                }
                State::NeedsComma => {
                    receiver.value_sep(current_token.span(), error);

                    state = State::NeedsKey;
                }
            },
            TokenKind::Equals => match state {
                State::NeedsKey => {
                    let fake_key = current_token.span().before();
                    let encoding = None;
                    receiver.simple_key(fake_key, encoding, error);

                    receiver.key_val_sep(current_token.span(), error);

                    empty = false;
                    state = State::NeedsValue;
                }
                State::NeedsEquals => {
                    receiver.key_val_sep(current_token.span(), error);

                    empty = false;
                    state = State::NeedsValue;
                }
                State::NeedsValue | State::NeedsComma => {
                    error.report_error(
                        ParseError::new("extra assignment between key-value pairs")
                            .with_context(inline_table_open.span())
                            .with_expected(state.expected())
                            .with_unexpected(current_token.span().before()),
                    );
                    receiver.error(current_token.span(), error);
                }
            },
            TokenKind::LeftCurlyBracket => match state {
                State::NeedsKey | State::NeedsComma => {
                    error.report_error(
                        ParseError::new("missing key for inline table element")
                            .with_context(inline_table_open.span())
                            .with_expected(state.expected())
                            .with_unexpected(current_token.span().before()),
                    );
                    receiver.error(current_token.span(), error);
                    ignore_to_value_close(tokens, TokenKind::RightCurlyBracket, receiver, error);
                }
                State::NeedsEquals => {
                    error.report_error(
                        ParseError::new("missing assignment between key-value pairs")
                            .with_context(inline_table_open.span())
                            .with_expected(state.expected())
                            .with_unexpected(current_token.span().before()),
                    );

                    on_inline_table_open(tokens, current_token, receiver, error);

                    empty = false;
                    state = State::NeedsComma;
                }
                State::NeedsValue => {
                    on_inline_table_open(tokens, current_token, receiver, error);

                    empty = false;
                    state = State::NeedsComma;
                }
            },
            TokenKind::RightCurlyBracket => {
                if !empty && !matches!(state, State::NeedsComma) {
                    let unexpected = tokens
                        .previous_tokens()
                        .find(|t| t.kind() == TokenKind::Comma)
                        .map(|t| t.span())
                        .unwrap_or_else(|| current_token.span().before());
                    error.report_error(
                        ParseError::new("trailing commas are not supported in inline tables")
                            .with_context(inline_table_open.span())
                            .with_expected(&[])
                            .with_unexpected(unexpected),
                    );
                }
                receiver.inline_table_close(current_token.span(), error);

                return;
            }
            TokenKind::LeftSquareBracket => match state {
                State::NeedsKey | State::NeedsComma => {
                    error.report_error(
                        ParseError::new("missing key for inline table element")
                            .with_context(inline_table_open.span())
                            .with_expected(state.expected())
                            .with_unexpected(current_token.span().before()),
                    );
                    receiver.error(current_token.span(), error);
                    ignore_to_value_close(tokens, TokenKind::RightSquareBracket, receiver, error);
                }
                State::NeedsEquals => {
                    error.report_error(
                        ParseError::new("missing assignment between key-value pairs")
                            .with_context(inline_table_open.span())
                            .with_expected(state.expected())
                            .with_unexpected(current_token.span().before()),
                    );

                    on_array_open(tokens, current_token, receiver, error);

                    empty = false;
                    state = State::NeedsComma;
                }
                State::NeedsValue => {
                    on_array_open(tokens, current_token, receiver, error);

                    empty = false;
                    state = State::NeedsComma;
                }
            },
            TokenKind::RightSquareBracket => match state {
                State::NeedsKey | State::NeedsEquals | State::NeedsComma => {
                    error.report_error(
                        ParseError::new("invalid inline table element")
                            .with_context(inline_table_open.span())
                            .with_expected(state.expected())
                            .with_unexpected(current_token.span().before()),
                    );
                    receiver.error(current_token.span(), error);
                }
                State::NeedsValue => {
                    error.report_error(
                        ParseError::new("missing array opening")
                            .with_context(current_token.span())
                            .with_expected(&[Expected::Literal("[")])
                            .with_unexpected(current_token.span().before()),
                    );

                    let _ = receiver.array_open(current_token.span().before(), error);
                    receiver.array_close(current_token.span(), error);

                    empty = false;
                    state = State::NeedsComma;
                }
            },
            TokenKind::LiteralString
            | TokenKind::BasicString
            | TokenKind::MlLiteralString
            | TokenKind::MlBasicString
            | TokenKind::Dot
            | TokenKind::Atom => match state {
                State::NeedsKey => {
                    if current_token.kind() == TokenKind::Dot {
                        receiver.simple_key(
                            current_token.span().before(),
                            current_token.kind().encoding(),
                            error,
                        );
                        seek(tokens, -1);
                        opt_dot_keys(tokens, receiver, error);
                        empty = false;
                        state = State::NeedsEquals;
                    } else {
                        receiver.simple_key(
                            current_token.span(),
                            current_token.kind().encoding(),
                            error,
                        );
                        opt_dot_keys(tokens, receiver, error);
                        empty = false;
                        state = State::NeedsEquals;
                    }
                }
                State::NeedsEquals => {
                    error.report_error(
                        ParseError::new("missing assignment between key-value pairs")
                            .with_context(inline_table_open.span())
                            .with_expected(state.expected())
                            .with_unexpected(current_token.span().before()),
                    );

                    on_scalar(tokens, current_token, receiver, error);

                    empty = false;
                    state = State::NeedsComma;
                }
                State::NeedsValue => {
                    on_scalar(tokens, current_token, receiver, error);

                    empty = false;
                    state = State::NeedsComma;
                }
                State::NeedsComma => {
                    error.report_error(
                        ParseError::new("missing comma between key-value pairs")
                            .with_context(inline_table_open.span())
                            .with_expected(state.expected())
                            .with_unexpected(current_token.span().before()),
                    );

                    if current_token.kind() == TokenKind::Dot {
                        receiver.simple_key(
                            current_token.span().before(),
                            current_token.kind().encoding(),
                            error,
                        );
                        seek(tokens, -1);
                        opt_dot_keys(tokens, receiver, error);
                        empty = false;
                        state = State::NeedsEquals;
                    } else {
                        receiver.simple_key(
                            current_token.span(),
                            current_token.kind().encoding(),
                            error,
                        );
                        opt_dot_keys(tokens, receiver, error);
                        empty = false;
                        state = State::NeedsEquals;
                    }
                }
            },
        }
    }

    let previous_span = tokens
        .previous_tokens()
        .find(|t| {
            !matches!(
                t.kind(),
                TokenKind::Whitespace | TokenKind::Comment | TokenKind::Newline | TokenKind::Eof
            )
        })
        .map(|t| t.span())
        .unwrap_or_default();
    error.report_error(
        ParseError::new("unclosed inline table")
            .with_context(inline_table_open.span())
            .with_expected(&[Expected::Literal("}")])
            .with_unexpected(previous_span.after()),
    );
    receiver.array_close(previous_span.after(), error);
}

/// Parse whitespace, if present
///
/// ```bnf
/// ws = *wschar
/// ```
fn opt_whitespace(
    tokens: &mut Stream<'_>,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    if let Some(ws_token) = next_token_if(tokens, |k| matches!(k, TokenKind::Whitespace)) {
        receiver.whitespace(ws_token.span(), error);
    }
}

/// Parse EOL decor, if present
///
/// ```bnf
/// toml = expression *( newline expression )
///
/// expression =  ws [ on_comment ]
/// expression =/ ws keyval ws [ on_comment ]
/// expression =/ ws table ws [ on_comment ]
///
/// ;; Whitespace
///
/// ws = *wschar
/// wschar =  %x20  ; Space
/// wschar =/ %x09  ; Horizontal tab
///
/// ;; Newline
///
/// newline =  %x0A     ; LF
/// newline =/ %x0D.0A  ; CRLF
///
/// ;; Comment
///
/// comment = comment-start-symbol *non-eol
/// ```
fn ws_comment_newline(
    tokens: &mut Stream<'_>,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    let mut first = None;
    while let Some(current_token) = tokens.next_token() {
        let first = first.get_or_insert(current_token.span());
        match current_token.kind() {
            TokenKind::Dot
            | TokenKind::Equals
            | TokenKind::Comma
            | TokenKind::LeftSquareBracket
            | TokenKind::RightSquareBracket
            | TokenKind::LeftCurlyBracket
            | TokenKind::RightCurlyBracket
            | TokenKind::LiteralString
            | TokenKind::BasicString
            | TokenKind::MlLiteralString
            | TokenKind::MlBasicString
            | TokenKind::Atom => {
                let context = first.append(current_token.span());
                error.report_error(
                    ParseError::new("unexpected key or value")
                        .with_context(context)
                        .with_expected(&[Expected::Literal("\n"), Expected::Literal("#")])
                        .with_unexpected(current_token.span().before()),
                );

                receiver.error(current_token.span(), error);
                ignore_to_newline(tokens, receiver, error);
                break;
            }
            TokenKind::Comment => {
                on_comment(tokens, current_token, receiver, error);
                break;
            }
            TokenKind::Whitespace => {
                receiver.whitespace(current_token.span(), error);
                continue;
            }
            TokenKind::Newline => {
                receiver.newline(current_token.span(), error);
                break;
            }
            TokenKind::Eof => {
                break;
            }
        }
    }
}

/// Start EOL from [`TokenKind::Comment`]
fn on_comment(
    tokens: &mut Stream<'_>,
    comment_token: &Token,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    receiver.comment(comment_token.span(), error);

    let Some(current_token) = tokens.next_token() else {
        return;
    };
    match current_token.kind() {
        TokenKind::Dot
        | TokenKind::Equals
        | TokenKind::Comma
        | TokenKind::LeftSquareBracket
        | TokenKind::RightSquareBracket
        | TokenKind::LeftCurlyBracket
        | TokenKind::RightCurlyBracket
        | TokenKind::Whitespace
        | TokenKind::Comment
        | TokenKind::LiteralString
        | TokenKind::BasicString
        | TokenKind::MlLiteralString
        | TokenKind::MlBasicString
        | TokenKind::Atom => {
            let context = comment_token.span().append(current_token.span());
            error.report_error(
                ParseError::new("unexpected content between comment and newline")
                    .with_context(context)
                    .with_expected(&[Expected::Literal("\n")])
                    .with_unexpected(current_token.span().before()),
            );

            receiver.error(current_token.span(), error);
            ignore_to_newline(tokens, receiver, error);
        }
        TokenKind::Newline => {
            receiver.newline(current_token.span(), error);
        }
        TokenKind::Eof => {}
    }
}

fn eof(tokens: &mut Stream<'_>, receiver: &mut dyn EventReceiver, error: &mut dyn ErrorSink) {
    let Some(current_token) = tokens.next_token() else {
        return;
    };

    match current_token.kind() {
        TokenKind::Dot
        | TokenKind::Equals
        | TokenKind::Comma
        | TokenKind::LeftSquareBracket
        | TokenKind::RightSquareBracket
        | TokenKind::LeftCurlyBracket
        | TokenKind::RightCurlyBracket
        | TokenKind::LiteralString
        | TokenKind::BasicString
        | TokenKind::MlLiteralString
        | TokenKind::MlBasicString
        | TokenKind::Atom
        | TokenKind::Comment
        | TokenKind::Whitespace
        | TokenKind::Newline => {
            error.report_error(
                ParseError::new("unexpected content")
                    .with_context(current_token.span())
                    .with_expected(&[])
                    .with_unexpected(current_token.span().before()),
            );

            receiver.error(current_token.span(), error);
            while let Some(current_token) = tokens.next_token() {
                if current_token.kind() == TokenKind::Eof {
                    continue;
                }
                receiver.error(current_token.span(), error);
            }
        }
        TokenKind::Eof => {}
    }
}

// Don't bother recovering until [`TokenKind::Newline`]
#[cold]
fn ignore_to_newline(
    tokens: &mut Stream<'_>,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    while let Some(current_token) = tokens.next_token() {
        match current_token.kind() {
            TokenKind::Dot
            | TokenKind::Equals
            | TokenKind::Comma
            | TokenKind::LeftSquareBracket
            | TokenKind::RightSquareBracket
            | TokenKind::LeftCurlyBracket
            | TokenKind::RightCurlyBracket
            | TokenKind::LiteralString
            | TokenKind::BasicString
            | TokenKind::MlLiteralString
            | TokenKind::MlBasicString
            | TokenKind::Atom => {
                receiver.error(current_token.span(), error);
            }
            TokenKind::Comment => {
                on_comment(tokens, current_token, receiver, error);
                break;
            }
            TokenKind::Whitespace => {
                receiver.whitespace(current_token.span(), error);
            }
            TokenKind::Newline => {
                receiver.newline(current_token.span(), error);
                break;
            }
            TokenKind::Eof => {
                break;
            }
        }
    }
}

/// Don't bother recovering until the matching [`TokenKind`]
///
/// Attempts to ignore nested `[]`, `{}`.
#[cold]
fn ignore_to_value_close(
    tokens: &mut Stream<'_>,
    closing_kind: TokenKind,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    let mut array_count: usize = 0;
    let mut inline_table_count: usize = 0;
    while let Some(current_token) = tokens.next_token() {
        match current_token.kind() {
            TokenKind::Dot
            | TokenKind::Equals
            | TokenKind::Comma
            | TokenKind::LiteralString
            | TokenKind::BasicString
            | TokenKind::MlLiteralString
            | TokenKind::MlBasicString
            | TokenKind::Atom => {
                receiver.error(current_token.span(), error);
            }
            TokenKind::Comment => {
                on_comment(tokens, current_token, receiver, error);
            }
            TokenKind::Whitespace => {
                receiver.whitespace(current_token.span(), error);
            }
            TokenKind::Newline => {
                receiver.newline(current_token.span(), error);
            }
            TokenKind::LeftSquareBracket => {
                receiver.error(current_token.span(), error);
                array_count += 1;
            }
            TokenKind::RightSquareBracket => {
                if array_count == 0 && current_token.kind() == closing_kind {
                    receiver.array_close(current_token.span(), error);
                    break;
                } else {
                    receiver.error(current_token.span(), error);
                    array_count = array_count.saturating_sub(1);
                }
            }
            TokenKind::LeftCurlyBracket => {
                receiver.error(current_token.span(), error);
                inline_table_count += 1;
            }
            TokenKind::RightCurlyBracket => {
                if inline_table_count == 0 && current_token.kind() == closing_kind {
                    receiver.inline_table_close(current_token.span(), error);
                    break;
                } else {
                    receiver.error(current_token.span(), error);
                    inline_table_count = inline_table_count.saturating_sub(1);
                }
            }
            TokenKind::Eof => {
                break;
            }
        }
    }
}

#[cold]
fn on_missing_key(
    tokens: &mut Stream<'_>,
    token: &Token,
    invalid_description: &'static str,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    error.report_error(
        ParseError::new(invalid_description)
            .with_context(token.span())
            .with_expected(&[Expected::Description("key")])
            .with_unexpected(token.span().before()),
    );

    if token.kind() == TokenKind::Eof {
    } else if token.kind() == TokenKind::Newline {
        receiver.newline(token.span(), error);
    } else if token.kind() == TokenKind::Comment {
        on_comment(tokens, token, receiver, error);
    } else {
        receiver.error(token.span(), error);
    }
}

#[cold]
fn on_missing_expression_key(
    tokens: &mut Stream<'_>,
    token: &Token,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    error.report_error(
        ParseError::new("invalid key-value pair")
            .with_context(token.span())
            .with_expected(&[Expected::Description("key")])
            .with_unexpected(token.span().before()),
    );

    receiver.error(token.span(), error);
    ignore_to_newline(tokens, receiver, error);
}

#[cold]
fn on_missing_std_table(
    tokens: &mut Stream<'_>,
    token: &Token,
    receiver: &mut dyn EventReceiver,
    error: &mut dyn ErrorSink,
) {
    error.report_error(
        ParseError::new("missing table open")
            .with_context(token.span())
            .with_expected(&[Expected::Literal("[")])
            .with_unexpected(token.span().before()),
    );

    receiver.error(token.span(), error);
    ignore_to_newline(tokens, receiver, error);
}

fn next_token_if<'i, F: Fn(TokenKind) -> bool>(
    tokens: &mut Stream<'i>,
    pred: F,
) -> Option<&'i Token> {
    match tokens.first() {
        Some(next) if pred(next.kind()) => tokens.next_token(),
        _ => None,
    }
}

fn seek(stream: &mut Stream<'_>, offset: isize) {
    let current = stream.checkpoint();
    stream.reset_to_start();
    let start = stream.checkpoint();
    let old_offset = current.offset_from(&start);
    let new_offset = (old_offset as isize).saturating_add(offset) as usize;
    if new_offset < stream.eof_offset() {
        #[cfg(feature = "unsafe")] // SAFETY: bounds were checked
        unsafe {
            stream.next_slice_unchecked(new_offset)
        };
        #[cfg(not(feature = "unsafe"))]
        stream.next_slice(new_offset);
    } else {
        stream.finish();
    }
}

const UNQUOTED_STRING: &str = "unquoted string";
