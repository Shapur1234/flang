//! Error types for compilation phases

use std::{io, num::ParseIntError, str::Utf8Error};

use thiserror::Error;

use crate::{
    lexer::token::{Ident, Literal, Op, PositionedToken, Token},
    parser::derivation::Expr as DExpr,
    semantic::ast::{Expr, Type},
};

/// Top-level compilation error wrapping errors from all phases
#[derive(Error, Debug, PartialEq)]
pub enum CompilationError {
    #[error("ReadError - {0}")]
    Read(ReadError),

    #[error("LexerError at {line}:{column} - {err}")]
    Lexer {
        err: LexerError,
        line: usize,
        column: usize,
    },

    #[error("ParserError - {0}")]
    Parser(ParserError),

    #[error("SemanticError - {0}")]
    Semantic(SemanticError),
}

///I/O or UTF-8 decoding errors
#[derive(Error, Debug)]
pub enum ReadError {
    #[error("{0:?}")]
    IoErr(#[from] io::Error),

    #[error("Invalid UTF8 sequence")]
    DecodeErr(#[from] Utf8Error),
}

impl PartialEq for ReadError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::DecodeErr(left), Self::DecodeErr(right)) => left == right,
            _ => false,
        }
    }
}

/// Lexer errors
#[derive(Error, Debug, PartialEq)]
pub enum LexerError {
    #[error("Unexpected character '{char}'")]
    UnexpectedChar { char: char },

    #[error("Identifier '{ident}' is invalid")]
    InvalidIdentifier { ident: String },

    #[error("String '{base}' is invalid base ({err})")]
    InvalidBaseParse { base: String, err: ParseIntError },
    #[error("Number '{base}' is invalid base")]
    InvalidBase { base: u32 },

    #[error("Base {base} Int literal '{literal}' is invalid ({err})")]
    IntParse {
        literal: String,
        base: u32,
        err: ParseIntError,
    },
    #[error("Base {base} Real whole part '{whole_part}' is invalid ({err})")]
    RealWholePartParse {
        whole_part: String,
        base: u32,
        err: ParseIntError,
    },
    #[error("Base {base} Real decimal part '{decimal_part}' is invalid ({err})")]
    RealDecimalPartParse {
        decimal_part: String,
        base: u32,
        err: ParseIntError,
    },
}

/// Parser errors
#[derive(Error, Debug, PartialEq)]
pub enum ParserError {
    #[error("Unexpected end of input")]
    UnexpectedEnd,

    #[error("Unexpected token '{:?}' at {}:{}", token.payload, token.line, token.column)]
    UnexpectedToken { token: PositionedToken },

    #[error("Expected '{expected:?}', but found '{:?} at {}:{}", found.payload, found.line, found.column)]
    ExpectedToken { expected: Token, found: PositionedToken },

    #[error("Expected end of input, but found '{:?} at {}{}'", found.payload, found.line, found.column)]
    ExpectedEnd { found: PositionedToken },
}

/// Semantic analysis errors
#[derive(Error, Debug, PartialEq)]
pub enum SemanticError {
    #[error("Identifier '{}' already defined", ident.0)]
    Redefinition { ident: Ident },

    #[error("Identifier '{}' undefined", ident.0)]
    Undefined { ident: Ident },

    #[error("Array bounds from '{from:?}' to '{to:?}' are invalid")]
    IllegalArrayBounds { from: Literal, to: Literal },

    #[error("Condition is of type '{ty:?}', expected '{:?}'", Type::Bool)]
    IllegalCondition { ty: Type },

    #[error("For from bound is of type '{ty:?}', expected '{:?}'", Type::Int)]
    IllegalForFrom { ty: Type },

    #[error("For to bound is of type '{ty:?}', expected '{:?}'", Type::Int)]
    IllegalForTo { ty: Type },

    #[error("For variable is of type '{ty:?}', expected '{:?}'", Type::Int)]
    IllegalForVar { ty: Type },

    #[error("Expr '{:?}' is no an lvalue", expr.kind)]
    NotLValue { expr: Expr },

    #[error("Cannot assign '{value:?}' into '{target:?}'")]
    AssignMismatch { value: Type, target: Type },

    #[error("Operator '{op:?}' cannot be applied to operand of type '{operand_ty:?}'")]
    IllegalUnaryOperand { op: Op, operand_ty: Type },

    #[error("Operator '{op:?}' must be applied to operands of same type, found '{left:?}' and '{right:?}'")]
    BinaryOpenradsTypeMismatch { op: Op, left: Type, right: Type },

    #[error("Operator '{op:?}' cannot be applied to operands of type '{operand_ty:?}'")]
    IllegalBinaryOperands { op: Op, operand_ty: Type },

    #[error("Index is of type '{ty:?}', expected '{:?}'", Type::Int)]
    IllegalArrayIndex { ty: Type },

    #[error("Cannot index into '{ty:?}', expected array")]
    IllegalIndexTarget { ty: Type },

    #[error("Cannot call '{callee:?}'. expected function or proc identifier")]
    IllegalCalleeTarget { callee: DExpr },

    #[error("Function '{}' expects {expected} arguments, got {got}", ident.0)]
    ArgCountMismatch { ident: Ident, expected: usize, got: usize },

    #[error("Function '{}' expects argument of type '{expected:?}' arguments at position {position}, got '{got:?}'", ident.0)]
    ArgTypeMismatch {
        ident: Ident,
        expected: Type,
        position: usize,
        got: Type,
    },

    #[error("Break statement outside of a loop")]
    BreakOutsideLoop,

    #[error("Continue statement outside of a loop")]
    ContinueOutsideLoop,
}
