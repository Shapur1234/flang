use std::{borrow::Cow, collections::BTreeMap};

use cphf::{OrderedMap, phf_ordered_map};

use crate::{
    lexer::token::Ident,
    semantic::ast::{Block, Func, Param, Type},
};

static NO_PARAM: [Param; 0] = [];
static BOOL_PARAM: [Param; 1] = [Param {
    name: Ident::of("value"),
    ty: Type::Bool,
}];
static INT_PARAM: [Param; 1] = [Param {
    name: Ident::of("value"),
    ty: Type::Int,
}];
static REAL_PARAM: [Param; 1] = [Param {
    name: Ident::of("value"),
    ty: Type::Real,
}];

/// Built-in function definitions
pub static BUILTIN_FUNCS: OrderedMap<Ident, Func> = phf_ordered_map! {Ident, Func;
    Ident::of("exit_ok") => Func {
        params: Cow::Borrowed(&[]),
        return_ty: Type::Void,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
    Ident::of("exit_fail") => Func {
        params: Cow::Borrowed(&[]),
        return_ty: Type::Void,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
    Ident::of("writeln") => Func {
        params: Cow::Borrowed(&INT_PARAM),
        return_ty: Type::Void,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
    Ident::of("readln") => Func {
        params: Cow::Borrowed(&NO_PARAM),
        return_ty: Type::Int,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
    Ident::of("writeln_real") => Func {
        params: Cow::Borrowed(&REAL_PARAM),
        return_ty: Type::Void,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
    Ident::of("readln_real") => Func {
        params: Cow::Borrowed(&NO_PARAM),
        return_ty: Type::Real,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
    Ident::of("bool_to_int") => Func {
        params: Cow::Borrowed(&BOOL_PARAM),
        return_ty: Type::Int,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
    Ident::of("bool_to_real") => Func {
        params: Cow::Borrowed(&BOOL_PARAM),
        return_ty: Type::Real,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
    Ident::of("int_to_bool") => Func {
        params: Cow::Borrowed(&INT_PARAM),
        return_ty: Type::Bool,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
    Ident::of("int_to_real") => Func {
        params: Cow::Borrowed(&INT_PARAM),
        return_ty: Type::Real,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
    Ident::of("real_to_bool") => Func {
        params: Cow::Borrowed(&REAL_PARAM),
        return_ty: Type::Bool,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
    Ident::of("real_to_int") => Func {
        params: Cow::Borrowed(&REAL_PARAM),
        return_ty: Type::Int,
        vars: BTreeMap::new(),
        block: Block { statements: vec![] },
    },
};
