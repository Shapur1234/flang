use crate::lexer::token::Ident;

/// A intermediate instruction for a stack-based computer
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Instruction {
    VarDecl { name: Ident, size: u64 },
    FuncDecl(String),
    EntrypointDecl,

    PushInt(i64),
    PushReal(i64),
    PushGlobalAddr(String),
    PushLocalAddr(i64),

    PushBasePtr,
    PopBasePtr,
    SetBasePtrToStackPtr,

    PushToSpecialReg1,
    PushToSpecialReg2,
    PopFromSpecialReg1,
    PopFromSpecialReg2,

    StAlloc(i64),
    StFree(i64),
    StMove { from: i64, bytes: i64, by: i64 },

    Read(u64),
    Write(u64),

    Pop(u64),

    AddInt,
    SubInt,
    MulInt,
    DivInt,
    ModInt,
    AddReal,
    SubReal,
    MulReal,
    DivReal,

    EqInt,
    NeqInt,
    LtInt,
    GtInt,
    LeqInt,
    GeqInt,
    EqReal,
    NeqReal,
    LtReal,
    GtReal,
    LeqReal,
    GeqReal,

    Not,
    And,
    Or,
    Xor,
    Imply,
    Equiv,

    Label(String),
    Jmp(String),
    JmpIfFalse(String),

    Call(String),
    Return,
}
