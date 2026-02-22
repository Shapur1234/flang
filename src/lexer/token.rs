use std::{
    borrow::{Borrow, Cow},
    fmt::{self, Debug},
};

use cphf::{ConstKey, Hasher, PhfKey, PhfKeyProxy};

/// A token with its source position
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PositionedToken {
    /// The token value
    pub payload: Token,
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub column: usize,
}

/// A token produced by the lexer
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Token {
    Keyword(Keyword),
    Ident(Ident),
    Literal(Literal),
    Op(Op),
    Punct(Punct),
}

impl Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Keyword(keyword) => keyword.fmt(f),
            Token::Ident(ident) => ident.fmt(f),
            Token::Literal(literal) => literal.fmt(f),
            Token::Op(op) => op.fmt(f),
            Token::Punct(punct) => punct.fmt(f),
        }
    }
}

/// Keywords of flang
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Keyword {
    Array,
    Break,
    Const,
    Continue,
    Do,
    Downto,
    Else,
    Exit,
    Fn,
    For,
    If,
    Of,
    Proc,
    Program,
    Then,
    To,
    Var,
    While,
    Int,
    Real,
    Bool,
}

/// An identifier in flang
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ident(pub Cow<'static, str>);

impl Ident {
    /// Creates an empty identifier
    pub const fn empty() -> Ident {
        Ident(Cow::Borrowed(""))
    }

    /// Creates an identifier from a static string
    pub const fn of(value: &'static str) -> Ident {
        Ident(Cow::Borrowed(value))
    }

    /// Checks if a string is a flang identifier
    pub fn is_valid_identifier(ident: &str) -> bool {
        if ident.is_empty() {
            return false;
        }

        let mut chars = ident.chars();
        let first_char = chars.next().unwrap();

        if !first_char.is_alphabetic() && first_char != '_' {
            return false;
        }

        let mut prev_was_underscore = first_char == '_';
        for char in chars {
            if char == '_' {
                if prev_was_underscore {
                    return false;
                }
                prev_was_underscore = true;
            } else if char.is_alphanumeric() {
                prev_was_underscore = false;
            } else {
                return false;
            }
        }

        true
    }
}

/// A literal value in flang
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Literal {
    Int(i64),
    Real(i64), // Contains bytes of f64 as i64
    Bool(bool),
}

impl Literal {
    /// Creates an empty literal (Int(0))
    pub const fn empty() -> Literal {
        Literal::Int(0)
    }

    /// Creates a Real literal from an f64 value
    pub const fn real(real: f64) -> Literal {
        Literal::Real(i64::from_le_bytes(real.to_le_bytes()))
    }

    /// Converts the internal i64 representation back to f64
    pub const fn real_to_f64(real: i64) -> f64 {
        f64::from_le_bytes(real.to_le_bytes())
    }
}

impl Debug for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(arg0) => f.debug_tuple("Int").field(arg0).finish(),
            Self::Real(arg0) => write!(f, "Real({})", f64::from_ne_bytes(arg0.to_ne_bytes())),
            Self::Bool(arg0) => f.debug_tuple("Bool").field(arg0).finish(),
        }
    }
}

/// Operation tokens
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op {
    /// `:=`
    Assign,
    /// `=`
    Eq,
    /// `!=`
    Neq,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Leq,
    /// `>=`
    Geq,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Times,
    /// `/`
    Div,
    /// `%`
    Mod,
    /// `not`
    Not,
    /// `and`
    And,
    /// `or`
    Or,
    /// `xor`
    Xor,
    /// `imply`
    Imply,
    /// `equiv`
    Equiv,
}

/// Punctuation tokens
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Punct {
    /// `(`
    LBracket,
    /// `)`
    RBracket,
    /// `{`
    LCurlyBracket,
    /// `}`
    RCurlyBracket,
    /// `[`
    LSquareBracket,
    /// `]`
    RSquareBracket,
    /// `:`
    Colon,
    /// `;`
    Semicolon,
    /// `,`
    Comma,
    /// `.`
    Dot,
    /// `..`
    DotDot,
    /// `->`
    Arrow,
}

// Cphf hashing for Token

impl Token {
    const fn kind_id(&self) -> u32 {
        match self {
            Token::Keyword(keyword) => match keyword {
                Keyword::Array => 0,
                Keyword::Break => 1,
                Keyword::Const => 2,
                Keyword::Continue => 3,
                Keyword::Do => 4,
                Keyword::Downto => 5,
                Keyword::Else => 6,
                Keyword::Exit => 7,
                Keyword::Fn => 8,
                Keyword::For => 9,
                Keyword::If => 10,
                Keyword::Of => 11,
                Keyword::Proc => 12,
                Keyword::Program => 13,
                Keyword::Then => 14,
                Keyword::To => 15,
                Keyword::Var => 16,
                Keyword::While => 17,
                Keyword::Int => 18,
                Keyword::Real => 19,
                Keyword::Bool => 20,
            },
            Token::Ident(_) => 100,
            Token::Literal(_) => 101,
            Token::Op(op) => match op {
                Op::Assign => 200,
                Op::Eq => 201,
                Op::Neq => 202,
                Op::Lt => 203,
                Op::Gt => 204,
                Op::Leq => 205,
                Op::Geq => 206,
                Op::Plus => 207,
                Op::Minus => 208,
                Op::Times => 209,
                Op::Div => 210,
                Op::Mod => 211,
                Op::Not => 212,
                Op::And => 213,
                Op::Or => 214,
                Op::Xor => 215,
                Op::Imply => 216,
                Op::Equiv => 217,
            },
            Token::Punct(punct) => match punct {
                Punct::LBracket => 300,
                Punct::RBracket => 301,
                Punct::LCurlyBracket => 302,
                Punct::RCurlyBracket => 303,
                Punct::LSquareBracket => 304,
                Punct::RSquareBracket => 305,
                Punct::Colon => 306,
                Punct::Semicolon => 307,
                Punct::Comma => 308,
                Punct::Dot => 309,
                Punct::DotDot => 310,
                Punct::Arrow => 311,
            },
        }
    }

    pub const fn kind_equals(&self, other: &Token) -> bool {
        self.kind_id() == other.kind_id()
    }
}

#[doc(hidden)]
pub struct TokenMarker;

impl PhfKey for Token {
    type ConstKey = TokenMarker;
}

impl ConstKey for TokenMarker {
    type PhfKey = Token;
}

impl TokenMarker {
    pub const fn pfh_hash(value: &Token, state: &mut Hasher) {
        <u32 as PhfKey>::ConstKey::pfh_hash(&value.kind_id(), state);
    }

    pub const fn pfh_eq(lhs: &Token, rhs: &Token) -> bool {
        lhs.kind_equals(rhs)
    }
}

impl<PK: ?Sized + Borrow<Token>> PhfKeyProxy<PK> for Token {
    fn pfh_hash(pk: &PK, state: &mut Hasher) {
        TokenMarker::pfh_hash(pk.borrow(), state);
    }

    fn pfh_eq(&self, other: &PK) -> bool {
        TokenMarker::pfh_eq(self, other.borrow())
    }
}

// Cphf hashing for Ident

#[doc(hidden)]
pub struct IdentMarker;

impl PhfKey for Ident {
    type ConstKey = IdentMarker;
}

impl ConstKey for IdentMarker {
    type PhfKey = Ident;
}

impl IdentMarker {
    // Taken from https://crates.io/crates/const-fnv1a-hash
    const fn fnv1a_hash_32(bytes: &[u8]) -> u32 {
        let prime = 16_777_619;
        let mut hash = 2_166_136_261;

        let mut i = 0;
        while i < bytes.len() {
            hash ^= bytes[i] as u32;
            hash = hash.wrapping_mul(prime);
            i += 1;
        }

        hash
    }

    pub const fn pfh_hash(value: &Ident, state: &mut Hasher) {
        <u32 as PhfKey>::ConstKey::pfh_hash(
            &IdentMarker::fnv1a_hash_32(match &value.0 {
                Cow::Borrowed(str) => str.as_bytes(),
                Cow::Owned(str) => str.as_bytes(),
            }),
            state,
        );
    }

    pub const fn pfh_eq(lhs: &Ident, rhs: &Ident) -> bool {
        match (&lhs.0, &rhs.0) {
            (Cow::Borrowed(lhs), Cow::Borrowed(rhs)) => *lhs == *rhs,
            (Cow::Borrowed(lhs), Cow::Owned(rhs)) => *lhs == rhs.as_str(),
            (Cow::Owned(lhs), Cow::Borrowed(rhs)) => lhs.as_str() == *rhs,
            (Cow::Owned(lhs), Cow::Owned(rhs)) => lhs.as_str() == rhs.as_str(),
        }
    }
}

impl<PK: ?Sized + Borrow<Ident>> PhfKeyProxy<PK> for Ident {
    fn pfh_hash(pk: &PK, state: &mut Hasher) {
        IdentMarker::pfh_hash(pk.borrow(), state);
    }

    fn pfh_eq(&self, other: &PK) -> bool {
        IdentMarker::pfh_eq(self, other.borrow())
    }
}
