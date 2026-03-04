use std::collections::BTreeSet;

use crate::CellAddress;
use crate::CellRange;
use crate::SpreadsheetArtifact;
use crate::SpreadsheetArtifactError;
use crate::SpreadsheetCellValue;

#[derive(Debug, Clone)]
enum Token {
    Number(f64),
    Cell(String),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Colon,
    Comma,
}

#[derive(Debug, Clone)]
enum Expr {
    Number(f64),
    Cell(CellAddress),
    Range(CellRange),
    UnaryMinus(Box<Expr>),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Function {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy)]
enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Debug, Clone)]
enum EvalValue {
    Scalar(Option<SpreadsheetCellValue>),
    Range(Vec<Option<SpreadsheetCellValue>>),
}

pub(crate) fn recalculate_workbook(artifact: &mut SpreadsheetArtifact) {
    let updates = artifact
        .sheets
        .iter()
        .enumerate()
        .flat_map(|(sheet_index, sheet)| {
            sheet.cells.iter().filter_map(move |(address, cell)| {
                cell.formula
                    .as_ref()
                    .map(|formula| (sheet_index, *address, formula.clone()))
            })
        })
        .map(|(sheet_index, address, formula)| {
            let mut stack = BTreeSet::new();
            let value = evaluate_formula(artifact, sheet_index, &formula, &mut stack)
                .unwrap_or_else(|error| {
                    Some(SpreadsheetCellValue::Error(map_error_to_code(&error)))
                });
            (sheet_index, address, value)
        })
        .collect::<Vec<_>>();

    for (sheet_index, address, value) in updates {
        if let Some(sheet) = artifact.sheets.get_mut(sheet_index)
            && let Some(cell) = sheet.cells.get_mut(&address)
        {
            cell.value = value;
        }
    }
}

fn evaluate_formula(
    artifact: &SpreadsheetArtifact,
    sheet_index: usize,
    formula: &str,
    stack: &mut BTreeSet<(usize, CellAddress)>,
) -> Result<Option<SpreadsheetCellValue>, SpreadsheetArtifactError> {
    let source = formula.trim().trim_start_matches('=');
    let tokens = tokenize(source)?;
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expression()?;
    if parser.has_remaining() {
        return Err(SpreadsheetArtifactError::Formula {
            location: formula.to_string(),
            message: "unexpected trailing tokens".to_string(),
        });
    }
    match evaluate_expr(artifact, sheet_index, &expr, stack)? {
        EvalValue::Scalar(value) => Ok(value),
        EvalValue::Range(_) => Err(SpreadsheetArtifactError::Formula {
            location: formula.to_string(),
            message: "range expressions are only allowed inside functions".to_string(),
        }),
    }
}

fn evaluate_expr(
    artifact: &SpreadsheetArtifact,
    sheet_index: usize,
    expr: &Expr,
    stack: &mut BTreeSet<(usize, CellAddress)>,
) -> Result<EvalValue, SpreadsheetArtifactError> {
    match expr {
        Expr::Number(value) => Ok(EvalValue::Scalar(Some(number_to_value(*value)))),
        Expr::Cell(address) => evaluate_cell_reference(artifact, sheet_index, *address, stack),
        Expr::Range(range) => {
            let sheet = artifact.sheets.get(sheet_index).ok_or_else(|| {
                SpreadsheetArtifactError::Formula {
                    location: range.to_a1(),
                    message: "sheet index was not found".to_string(),
                }
            })?;
            let values = range
                .addresses()
                .map(|address| sheet.get_cell(address).and_then(|cell| cell.value.clone()))
                .collect::<Vec<_>>();
            Ok(EvalValue::Range(values))
        }
        Expr::UnaryMinus(inner) => {
            let value = evaluate_scalar(artifact, sheet_index, inner, stack)?;
            Ok(EvalValue::Scalar(match value {
                None => Some(SpreadsheetCellValue::Integer(0)),
                Some(SpreadsheetCellValue::Integer(value)) => {
                    Some(SpreadsheetCellValue::Integer(-value))
                }
                Some(SpreadsheetCellValue::Float(value)) => {
                    Some(SpreadsheetCellValue::Float(-value))
                }
                Some(SpreadsheetCellValue::Error(value)) => {
                    Some(SpreadsheetCellValue::Error(value))
                }
                Some(_) => Some(SpreadsheetCellValue::Error("#VALUE!".to_string())),
            }))
        }
        Expr::Binary { op, left, right } => {
            let left = evaluate_scalar(artifact, sheet_index, left, stack)?;
            let right = evaluate_scalar(artifact, sheet_index, right, stack)?;
            Ok(EvalValue::Scalar(Some(apply_binary_op(*op, left, right)?)))
        }
        Expr::Function { name, args } => {
            let mut numeric = Vec::new();
            for arg in args {
                match evaluate_expr(artifact, sheet_index, arg, stack)? {
                    EvalValue::Scalar(value) => {
                        if let Some(number) = scalar_to_number(value.clone())? {
                            numeric.push(number);
                        }
                    }
                    EvalValue::Range(values) => {
                        for value in values {
                            if let Some(number) = scalar_to_number(value.clone())? {
                                numeric.push(number);
                            }
                        }
                    }
                }
            }
            let upper = name.to_ascii_uppercase();
            let result = match upper.as_str() {
                "SUM" => numeric.iter().sum::<f64>(),
                "AVERAGE" => {
                    if numeric.is_empty() {
                        return Ok(EvalValue::Scalar(None));
                    }
                    numeric.iter().sum::<f64>() / numeric.len() as f64
                }
                "MIN" => numeric.iter().copied().reduce(f64::min).unwrap_or(0.0),
                "MAX" => numeric.iter().copied().reduce(f64::max).unwrap_or(0.0),
                _ => {
                    return Ok(EvalValue::Scalar(Some(SpreadsheetCellValue::Error(
                        "#NAME?".to_string(),
                    ))));
                }
            };
            Ok(EvalValue::Scalar(Some(number_to_value(result))))
        }
    }
}

fn evaluate_scalar(
    artifact: &SpreadsheetArtifact,
    sheet_index: usize,
    expr: &Expr,
    stack: &mut BTreeSet<(usize, CellAddress)>,
) -> Result<Option<SpreadsheetCellValue>, SpreadsheetArtifactError> {
    match evaluate_expr(artifact, sheet_index, expr, stack)? {
        EvalValue::Scalar(value) => Ok(value),
        EvalValue::Range(_) => Err(SpreadsheetArtifactError::Formula {
            location: format!("{expr:?}"),
            message: "expected a scalar expression".to_string(),
        }),
    }
}

fn evaluate_cell_reference(
    artifact: &SpreadsheetArtifact,
    sheet_index: usize,
    address: CellAddress,
    stack: &mut BTreeSet<(usize, CellAddress)>,
) -> Result<EvalValue, SpreadsheetArtifactError> {
    let Some(sheet) = artifact.sheets.get(sheet_index) else {
        return Err(SpreadsheetArtifactError::Formula {
            location: address.to_a1(),
            message: "sheet index was not found".to_string(),
        });
    };
    let key = (sheet_index, address);
    if !stack.insert(key) {
        return Ok(EvalValue::Scalar(Some(SpreadsheetCellValue::Error(
            "#CYCLE!".to_string(),
        ))));
    }

    let value = if let Some(cell) = sheet.get_cell(address) {
        if let Some(formula) = &cell.formula {
            evaluate_formula(artifact, sheet_index, formula, stack)?
        } else {
            cell.value.clone()
        }
    } else {
        None
    };
    stack.remove(&key);
    Ok(EvalValue::Scalar(value))
}

fn apply_binary_op(
    op: BinaryOp,
    left: Option<SpreadsheetCellValue>,
    right: Option<SpreadsheetCellValue>,
) -> Result<SpreadsheetCellValue, SpreadsheetArtifactError> {
    if let Some(SpreadsheetCellValue::Error(value)) = &left {
        return Ok(SpreadsheetCellValue::Error(value.clone()));
    }
    if let Some(SpreadsheetCellValue::Error(value)) = &right {
        return Ok(SpreadsheetCellValue::Error(value.clone()));
    }

    let left = scalar_to_number(left)?;
    let right = scalar_to_number(right)?;
    let left = left.unwrap_or(0.0);
    let right = right.unwrap_or(0.0);
    let result = match op {
        BinaryOp::Add => left + right,
        BinaryOp::Subtract => left - right,
        BinaryOp::Multiply => left * right,
        BinaryOp::Divide => {
            if right == 0.0 {
                return Ok(SpreadsheetCellValue::Error("#DIV/0!".to_string()));
            }
            left / right
        }
    };
    Ok(number_to_value(result))
}

fn scalar_to_number(
    value: Option<SpreadsheetCellValue>,
) -> Result<Option<f64>, SpreadsheetArtifactError> {
    match value {
        None => Ok(None),
        Some(SpreadsheetCellValue::Integer(value)) => Ok(Some(value as f64)),
        Some(SpreadsheetCellValue::Float(value)) => Ok(Some(value)),
        Some(SpreadsheetCellValue::Bool(value)) => Ok(Some(if value { 1.0 } else { 0.0 })),
        Some(SpreadsheetCellValue::Error(value)) => Err(SpreadsheetArtifactError::Formula {
            location: value,
            message: "encountered error value".to_string(),
        }),
        Some(other) => Err(SpreadsheetArtifactError::Formula {
            location: format!("{other:?}"),
            message: "value is not numeric".to_string(),
        }),
    }
}

fn number_to_value(number: f64) -> SpreadsheetCellValue {
    if number.fract() == 0.0 {
        SpreadsheetCellValue::Integer(number as i64)
    } else {
        SpreadsheetCellValue::Float(number)
    }
}

fn map_error_to_code(error: &SpreadsheetArtifactError) -> String {
    match error {
        SpreadsheetArtifactError::Formula { message, .. } => {
            if message.contains("cycle") {
                "#CYCLE!".to_string()
            } else if message.contains("not numeric") || message.contains("scalar") {
                "#VALUE!".to_string()
            } else {
                "#ERROR!".to_string()
            }
        }
        SpreadsheetArtifactError::InvalidAddress { .. } => "#REF!".to_string(),
        _ => "#ERROR!".to_string(),
    }
}

fn tokenize(source: &str) -> Result<Vec<Token>, SpreadsheetArtifactError> {
    let chars = source.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    let mut tokens = Vec::new();
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        match ch {
            '+' => {
                tokens.push(Token::Plus);
                index += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                index += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                index += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                index += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                index += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                index += 1;
            }
            ':' => {
                tokens.push(Token::Colon);
                index += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                index += 1;
            }
            '0'..='9' | '.' => {
                let start = index;
                index += 1;
                while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.')
                {
                    index += 1;
                }
                let number = source[start..index].parse::<f64>().map_err(|_| {
                    SpreadsheetArtifactError::Formula {
                        location: source.to_string(),
                        message: "invalid numeric literal".to_string(),
                    }
                })?;
                tokens.push(Token::Number(number));
            }
            'A'..='Z' | 'a'..='z' | '_' => {
                let start = index;
                index += 1;
                while index < chars.len()
                    && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
                {
                    index += 1;
                }
                let text = source[start..index].to_string();
                if text.chars().any(|part| part.is_ascii_digit())
                    && text.chars().any(|part| part.is_ascii_alphabetic())
                {
                    tokens.push(Token::Cell(text));
                } else {
                    tokens.push(Token::Ident(text));
                }
            }
            other => {
                return Err(SpreadsheetArtifactError::Formula {
                    location: source.to_string(),
                    message: format!("unsupported token `{other}`"),
                });
            }
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    fn has_remaining(&self) -> bool {
        self.index < self.tokens.len()
    }

    fn parse_expression(&mut self) -> Result<Expr, SpreadsheetArtifactError> {
        let mut expr = self.parse_term()?;
        while let Some(token) = self.peek() {
            let op = match token {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Subtract,
                _ => break,
            };
            self.index += 1;
            let right = self.parse_term()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, SpreadsheetArtifactError> {
        let mut expr = self.parse_factor()?;
        while let Some(token) = self.peek() {
            let op = match token {
                Token::Star => BinaryOp::Multiply,
                Token::Slash => BinaryOp::Divide,
                _ => break,
            };
            self.index += 1;
            let right = self.parse_factor()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, SpreadsheetArtifactError> {
        match self.peek() {
            Some(Token::Minus) => {
                self.index += 1;
                Ok(Expr::UnaryMinus(Box::new(self.parse_factor()?)))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, SpreadsheetArtifactError> {
        match self.next().cloned() {
            Some(Token::Number(value)) => Ok(Expr::Number(value)),
            Some(Token::Cell(address)) => {
                let start = CellAddress::parse(&address)?;
                if matches!(self.peek(), Some(Token::Colon)) {
                    self.index += 1;
                    let Some(Token::Cell(end)) = self.next().cloned() else {
                        return Err(SpreadsheetArtifactError::Formula {
                            location: address,
                            message: "expected cell after `:`".to_string(),
                        });
                    };
                    Ok(Expr::Range(CellRange::from_start_end(
                        start,
                        CellAddress::parse(&end)?,
                    )))
                } else {
                    Ok(Expr::Cell(start))
                }
            }
            Some(Token::Ident(name)) => {
                if !matches!(self.next(), Some(Token::LParen)) {
                    return Err(SpreadsheetArtifactError::Formula {
                        location: name,
                        message: "expected `(` after function name".to_string(),
                    });
                }
                let mut args = Vec::new();
                if !matches!(self.peek(), Some(Token::RParen)) {
                    loop {
                        args.push(self.parse_expression()?);
                        if matches!(self.peek(), Some(Token::Comma)) {
                            self.index += 1;
                            continue;
                        }
                        break;
                    }
                }
                if !matches!(self.next(), Some(Token::RParen)) {
                    return Err(SpreadsheetArtifactError::Formula {
                        location: name,
                        message: "expected `)`".to_string(),
                    });
                }
                Ok(Expr::Function { name, args })
            }
            Some(Token::LParen) => {
                let expr = self.parse_expression()?;
                if !matches!(self.next(), Some(Token::RParen)) {
                    return Err(SpreadsheetArtifactError::Formula {
                        location: format!("{expr:?}"),
                        message: "expected `)`".to_string(),
                    });
                }
                Ok(expr)
            }
            other => Err(SpreadsheetArtifactError::Formula {
                location: format!("{other:?}"),
                message: "unexpected token".to_string(),
            }),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.index);
        self.index += usize::from(token.is_some());
        token
    }
}
