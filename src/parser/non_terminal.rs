use cphf::{ConstKey, Hasher, PhfKey, PhfKeyProxy};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NonTerminal {
    S,
    Decls,
    ConstDecl,
    ConstDeclBody,
    VarDecl,
    VarDeclBody,
    ProcDecl,
    FnDecl,
    VarDecls,
    Type,
    Args,
    ArgsTail,
    ArgsNext,
    VarNames,
    VarNamesTail,
    Block,
    Statements,
    AfterStatement,
    Statement,
    IfStatement,
    IfTail,
    WhileStatement,
    ForStatement,
    ForDir,
    ExprStatementTail,
    CalledArgs,
    CalledArgsTail,
    CalledNext,
    Expr,
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E1Tail,
    E2Tail,
    E3Tail,
    E4Tail,
    E5Tail,
    E6Tail,
    Atom,
    AtomTail,
}

#[doc(hidden)]
pub struct NonTerminalMarker;

impl PhfKey for NonTerminal {
    type ConstKey = NonTerminalMarker;
}

impl ConstKey for NonTerminalMarker {
    type PhfKey = NonTerminal;
}

const fn nonterminal_kind_id(value: &NonTerminal) -> u32 {
    match value {
        NonTerminal::S => 0,
        NonTerminal::Decls => 1,
        NonTerminal::ConstDecl => 2,
        NonTerminal::ConstDeclBody => 3,
        NonTerminal::VarDecl => 4,
        NonTerminal::VarDeclBody => 5,
        NonTerminal::ProcDecl => 6,
        NonTerminal::FnDecl => 7,
        NonTerminal::VarDecls => 8,
        NonTerminal::Type => 9,
        NonTerminal::Args => 10,
        NonTerminal::ArgsTail => 11,
        NonTerminal::ArgsNext => 12,
        NonTerminal::VarNames => 13,
        NonTerminal::VarNamesTail => 14,
        NonTerminal::Block => 15,
        NonTerminal::Statements => 16,
        NonTerminal::AfterStatement => 17,
        NonTerminal::Statement => 18,
        NonTerminal::IfStatement => 19,
        NonTerminal::IfTail => 20,
        NonTerminal::WhileStatement => 21,
        NonTerminal::ForStatement => 22,
        NonTerminal::ForDir => 23,
        NonTerminal::ExprStatementTail => 24,
        NonTerminal::CalledArgs => 26,
        NonTerminal::CalledArgsTail => 27,
        NonTerminal::CalledNext => 28,
        NonTerminal::Expr => 29,
        NonTerminal::E1 => 30,
        NonTerminal::E2 => 31,
        NonTerminal::E3 => 32,
        NonTerminal::E4 => 33,
        NonTerminal::E5 => 34,
        NonTerminal::E6 => 35,
        NonTerminal::E1Tail => 36,
        NonTerminal::E2Tail => 37,
        NonTerminal::E3Tail => 38,
        NonTerminal::E4Tail => 39,
        NonTerminal::E5Tail => 40,
        NonTerminal::E6Tail => 41,
        NonTerminal::Atom => 42,
        NonTerminal::AtomTail => 43,
    }
}

impl NonTerminalMarker {
    pub const fn pfh_hash(value: &NonTerminal, state: &mut Hasher) {
        let kind = nonterminal_kind_id(value);
        <u32 as PhfKey>::ConstKey::pfh_hash(&kind, state);
    }

    pub const fn pfh_eq(lhs: &NonTerminal, rhs: &NonTerminal) -> bool {
        nonterminal_kind_id(lhs) == nonterminal_kind_id(rhs)
    }
}

impl<PK: ?Sized + std::borrow::Borrow<NonTerminal>> PhfKeyProxy<PK> for NonTerminal {
    fn pfh_hash(pk: &PK, state: &mut Hasher) {
        NonTerminalMarker::pfh_hash(pk.borrow(), state);
    }

    fn pfh_eq(&self, other: &PK) -> bool {
        NonTerminalMarker::pfh_eq(self, other.borrow())
    }
}
