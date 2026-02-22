use std::collections::{BTreeMap, HashSet};

use crate::{
    error::{CompilationError, SemanticError},
    lexer::token::{Ident, Literal, Op},
    parser::derivation::{
        Block as DBlock, Decl as DDecl, Expr as DExpr, ForDirection as DForDirection, Param as DParam, Program,
        Statement as DStatement, Type as DType,
    },
    semantic::{
        BUILTIN_FUNCS,
        ast::{Ast, BinaryOp, Block, Expr, ExprKind, ForDirection, Func, Param, Statement, Type, UnaryOp},
    },
};

/// Performs semantic analysis on the derivation tree
///
/// Checks types, resolves identifiers, and builds the AST
pub fn check(program: Program) -> Result<Ast, CompilationError> {
    let Program { name, decls, block } = program;

    let global_idents = global_idents(&decls).map_err(CompilationError::Semantic)?;
    let global_decls = check_decls(decls, &global_idents).map_err(CompilationError::Semantic)?;
    let funcs = check_funcs(&global_decls).map_err(CompilationError::Semantic)?;

    let entrypoint = {
        let main_checker = TypeChecker {
            global_decls: &global_decls,
            local_vars: None,
            params: None,
            func_ident: None,
            return_ty: None,
            in_loop: false,
        };

        main_checker.check_block(&block).map_err(CompilationError::Semantic)?
    };

    Ok(Ast {
        name,
        vars: global_decls.vars,
        funcs,
        entrypoint,
    })
}

fn global_idents(decls: &[DDecl]) -> Result<HashSet<Ident>, SemanticError> {
    let mut out = HashSet::new();

    out.reserve(BUILTIN_FUNCS.len());
    for name in BUILTIN_FUNCS.keys() {
        out.insert(name.clone());
    }

    for decl in decls {
        match decl {
            DDecl::Const { names, .. } | DDecl::Var { names, .. } => {
                for name in names {
                    if !out.insert(name.clone()) {
                        return Err(SemanticError::Redefinition { ident: name.clone() });
                    }
                }
            }
            DDecl::Proc { name, .. } | DDecl::Fn { name, .. } => {
                if !out.insert(name.clone()) {
                    return Err(SemanticError::Redefinition { ident: name.clone() });
                }
            }
        }
    }

    Ok(out)
}

fn check_decls(decls: Vec<DDecl>, global_idents: &HashSet<Ident>) -> Result<GlobalDDecls, SemanticError> {
    let mut global_decls = GlobalDDecls {
        consts: BTreeMap::new(),
        vars: BTreeMap::new(),
        funcs: BTreeMap::new(),
    };

    for (name, func) in &BUILTIN_FUNCS {
        global_decls.funcs.insert(
            name.clone(),
            FuncDDecl {
                params: func.params.to_vec(),
                return_ty: func.return_ty.clone(),
                vars: BTreeMap::new(),
                block: DBlock { statements: vec![] },
            },
        );
    }

    for decl in decls {
        match decl {
            DDecl::Const { names, value } => {
                for name in names {
                    global_decls.consts.insert(name, value.clone());
                }
            }
            DDecl::Var { names, ty } => {
                let ty: Type = TryFrom::try_from(ty)?;
                for name in names {
                    global_decls.vars.insert(name, ty.clone());
                }
            }
            DDecl::Proc {
                name,
                params,
                locals,
                block,
            } => {
                let (name, func_decl) = check_function(global_idents, name, params, locals, block, None)?;
                global_decls.funcs.insert(name, func_decl);
            }

            DDecl::Fn {
                name,
                params,
                return_type,
                locals,
                block,
            } => {
                let (name, func_decl) = check_function(global_idents, name, params, locals, block, Some(return_type))?;
                global_decls.funcs.insert(name, func_decl);
            }
        }
    }

    Ok(global_decls)
}

#[derive(Debug, Clone)]
struct FuncDDecl {
    params: Vec<Param>,
    return_ty: Type,
    vars: BTreeMap<Ident, Type>,
    block: DBlock,
}

fn check_function(
    global_idents: &HashSet<Ident>,
    name: Ident,
    params: Vec<DParam>,
    locals: Vec<DDecl>,
    block: DBlock,
    return_ty: Option<DType>,
) -> Result<(Ident, FuncDDecl), SemanticError> {
    let mut local_idents = HashSet::new();

    let return_ty = match return_ty {
        Some(return_ty) => TryFrom::try_from(return_ty)?,
        None => Type::Void,
    };

    let params = {
        let mut out = Vec::with_capacity(params.len());

        for DParam { ty, names } in params {
            let ty: Type = TryFrom::try_from(ty)?;

            for name in names {
                if global_idents.contains(&name) || !local_idents.insert(name.clone()) {
                    return Err(SemanticError::Redefinition { ident: name.clone() });
                }
                out.push(Param { name, ty: ty.clone() });
            }
        }
        out
    };

    let vars = {
        let mut out = BTreeMap::new();
        for local in locals {
            match local {
                DDecl::Var { names, ty } => {
                    let ty: Type = TryFrom::try_from(ty)?;

                    for name in names {
                        if global_idents.contains(&name) || !local_idents.insert(name.clone()) {
                            return Err(SemanticError::Redefinition { ident: name.clone() });
                        }
                        out.insert(name, ty.clone());
                    }
                }
                _ => panic!("Declaration '{local:?}' of local variable is not a variable"),
            }
        }
        out
    };

    Ok((
        name,
        FuncDDecl {
            params,
            return_ty,
            vars,
            block,
        },
    ))
}

struct GlobalDDecls {
    consts: BTreeMap<Ident, Literal>,
    vars: BTreeMap<Ident, Type>,
    funcs: BTreeMap<Ident, FuncDDecl>,
}

fn check_funcs(global_decls: &GlobalDDecls) -> Result<BTreeMap<Ident, Func>, SemanticError> {
    let mut out = BTreeMap::new();
    for (ident, func_decl) in &global_decls.funcs {
        let checker = TypeChecker {
            global_decls,
            local_vars: Some(&func_decl.vars),
            params: Some(&func_decl.params),
            func_ident: Some(ident),
            return_ty: Some(&func_decl.return_ty),
            in_loop: false,
        };

        let ast_block = checker.check_block(&func_decl.block)?;

        out.insert(
            ident.clone(),
            Func {
                params: func_decl.params.clone().into(),
                return_ty: func_decl.return_ty.clone(),
                vars: func_decl.vars.clone(),
                block: ast_block,
            },
        );
    }
    Ok(out)
}

#[derive(Clone)]
struct TypeChecker<'a> {
    global_decls: &'a GlobalDDecls,
    local_vars: Option<&'a BTreeMap<Ident, Type>>,
    params: Option<&'a [Param]>,
    func_ident: Option<&'a Ident>,
    return_ty: Option<&'a Type>,
    in_loop: bool,
}

impl<'a> TypeChecker<'a> {
    fn in_loop(&self) -> TypeChecker<'a> {
        TypeChecker {
            in_loop: true,
            ..self.clone()
        }
    }
}

impl TypeChecker<'_> {
    fn check_block(&self, block: &DBlock) -> Result<Block, SemanticError> {
        let mut statements = Vec::with_capacity(block.statements.len());
        for stmt in &block.statements {
            statements.push(self.check_statement(stmt)?);
        }
        Ok(Block { statements })
    }

    fn check_statement(&self, stmt: &DStatement) -> Result<Statement, SemanticError> {
        match stmt {
            DStatement::Block(b) => Ok(Statement::Block(self.check_block(b)?)),
            DStatement::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond = self.check_expr(cond)?;
                if cond.ty != Type::Bool {
                    return Err(SemanticError::IllegalCondition { ty: cond.ty });
                }

                Ok(Statement::If {
                    cond,
                    then_branch: Box::new(self.check_statement(then_branch)?),
                    else_branch: match else_branch {
                        Some(else_branch) => Some(Box::new(self.check_statement(else_branch)?)),
                        None => None,
                    },
                })
            }
            DStatement::While { cond, body } => {
                let cond = self.check_expr(cond)?;
                if cond.ty != Type::Bool {
                    return Err(SemanticError::IllegalCondition { ty: cond.ty });
                }

                Ok(Statement::While {
                    cond,
                    body: Box::new(self.in_loop().check_statement(body)?),
                })
            }
            DStatement::For {
                name: var_name,
                direction,
                from,
                to,
                body,
            } => {
                let from = self.check_expr(from)?;
                if from.ty != Type::Int {
                    return Err(SemanticError::IllegalForFrom { ty: from.ty });
                }

                let to = self.check_expr(to)?;
                if to.ty != Type::Int {
                    return Err(SemanticError::IllegalForTo { ty: to.ty });
                }

                let var = self.check_expr(&DExpr::Ident(var_name.clone()))?;
                if var.ty != Type::Int {
                    return Err(SemanticError::IllegalForVar { ty: var.ty });
                }

                if !var.is_lvalue {
                    return Err(SemanticError::NotLValue { expr: var });
                }

                Ok(Statement::For {
                    name: var_name.clone(),
                    direction: match direction {
                        DForDirection::To => ForDirection::To,
                        DForDirection::Downto => ForDirection::Downto,
                    },
                    from,
                    to,
                    body: Box::new(self.in_loop().check_statement(body)?),
                })
            }
            DStatement::Assign { target, value } => {
                let target = self.check_expr(target)?;
                let value = self.check_expr(value)?;

                if !target.is_lvalue {
                    return Err(SemanticError::NotLValue { expr: target });
                }

                if target.ty != value.ty {
                    return Err(SemanticError::AssignMismatch {
                        value: value.ty,
                        target: target.ty,
                    });
                }
                Ok(Statement::Assign { target, value })
            }
            DStatement::Expr(e) => Ok(Statement::Expr(self.check_expr(e)?)),
            DStatement::Break => {
                if !self.in_loop {
                    return Err(SemanticError::BreakOutsideLoop);
                }
                Ok(Statement::Break)
            }
            DStatement::Continue => {
                if !self.in_loop {
                    return Err(SemanticError::ContinueOutsideLoop);
                }
                Ok(Statement::Continue)
            }
            DStatement::Exit => Ok(Statement::Exit),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn check_expr(&self, expr: &DExpr) -> Result<Expr, SemanticError> {
        match expr {
            DExpr::Ident(ident) => {
                if let Some(func_ident) = self.func_ident
                    && ident == func_ident
                {
                    Ok(Expr {
                        kind: ExprKind::Return,
                        ty: self.return_ty.cloned().unwrap_or(Type::Void),
                        is_lvalue: true,
                    })
                } else if let Some(constant) = self.global_decls.consts.get(ident) {
                    Ok(Expr {
                        kind: ExprKind::Literal(constant.clone()),
                        ty: literal_type(constant),
                        is_lvalue: false,
                    })
                } else {
                    Ok(Expr {
                        kind: ExprKind::Var(ident.clone()),
                        ty: self.get_ident_type(ident)?,
                        is_lvalue: true,
                    })
                }
            }
            DExpr::Literal(lit) => Ok(Expr {
                kind: ExprKind::Literal(lit.clone()),
                ty: match lit {
                    Literal::Int(_) => Type::Int,
                    Literal::Real(_) => Type::Real,
                    Literal::Bool(_) => Type::Bool,
                },
                is_lvalue: false,
            }),
            DExpr::Unary { op, expr: inner } => {
                let inner = self.check_expr(inner)?;
                let unary_op = match op {
                    Op::Minus => UnaryOp::Minus,
                    Op::Plus => UnaryOp::Plus,
                    Op::Not => UnaryOp::Not,
                    _ => panic!("'{op:?} is not a unary operator"),
                };

                match (&unary_op, inner.ty.clone()) {
                    (UnaryOp::Minus | UnaryOp::Plus, return_ty @ (Type::Int | Type::Real)) => Ok(Expr {
                        kind: ExprKind::Unary {
                            op: unary_op,
                            expr: Box::new(inner),
                        },
                        ty: return_ty,
                        is_lvalue: false,
                    }),
                    (UnaryOp::Not, return_ty @ (Type::Bool | Type::Int)) => Ok(Expr {
                        kind: ExprKind::Unary {
                            op: unary_op,
                            expr: Box::new(inner),
                        },
                        ty: return_ty.clone(),
                        is_lvalue: false,
                    }),
                    _ => Err(SemanticError::IllegalUnaryOperand {
                        op: op.clone(),
                        operand_ty: inner.ty,
                    }),
                }
            }
            DExpr::Binary { left, op, right } => {
                let left = self.check_expr(left)?;
                let right = self.check_expr(right)?;

                if left.ty != right.ty {
                    return Err(SemanticError::BinaryOpenradsTypeMismatch {
                        op: op.clone(),
                        left: left.ty,
                        right: right.ty,
                    });
                }

                let (ast_op, result_ty) = match op {
                    Op::Eq | Op::Neq | Op::Lt | Op::Gt | Op::Leq | Op::Geq => {
                        if !matches!(left.ty, Type::Int | Type::Real | Type::Bool) {
                            return Err(SemanticError::IllegalBinaryOperands {
                                op: op.clone(),
                                operand_ty: left.ty,
                            });
                        }

                        let binary_op = match op {
                            Op::Eq => BinaryOp::Eq,
                            Op::Neq => BinaryOp::Neq,
                            Op::Lt => BinaryOp::Lt,
                            Op::Gt => BinaryOp::Gt,
                            Op::Leq => BinaryOp::Leq,
                            Op::Geq => BinaryOp::Geq,
                            _ => unreachable!(),
                        };
                        (binary_op, Type::Bool)
                    }

                    Op::Plus | Op::Minus | Op::Times | Op::Div => {
                        if !matches!(left.ty, Type::Int | Type::Real) {
                            return Err(SemanticError::IllegalBinaryOperands {
                                op: op.clone(),
                                operand_ty: left.ty,
                            });
                        }

                        let binary_op = match op {
                            Op::Plus => BinaryOp::Plus,
                            Op::Minus => BinaryOp::Minus,
                            Op::Times => BinaryOp::Times,
                            Op::Div => BinaryOp::Div,
                            _ => unreachable!(),
                        };
                        (binary_op, left.ty.clone())
                    }

                    Op::Mod => {
                        if !matches!(left.ty, Type::Int) {
                            return Err(SemanticError::IllegalBinaryOperands {
                                op: op.clone(),
                                operand_ty: left.ty,
                            });
                        }

                        let binary_op = match op {
                            Op::Mod => BinaryOp::Mod,
                            _ => unreachable!(),
                        };
                        (binary_op, left.ty.clone())
                    }

                    Op::And | Op::Or | Op::Xor | Op::Imply | Op::Equiv => {
                        if !matches!(left.ty, Type::Int | Type::Bool) {
                            return Err(SemanticError::IllegalBinaryOperands {
                                op: op.clone(),
                                operand_ty: left.ty,
                            });
                        }

                        let binary_op = match op {
                            Op::And => BinaryOp::And,
                            Op::Or => BinaryOp::Or,
                            Op::Xor => BinaryOp::Xor,
                            Op::Imply => BinaryOp::Imply,
                            Op::Equiv => BinaryOp::Equiv,
                            _ => unreachable!(),
                        };
                        (binary_op, left.ty.clone())
                    }
                    _ => panic!("'{op:?} is not a binary operator"),
                };

                Ok(Expr {
                    kind: ExprKind::Binary {
                        left: Box::new(left),
                        op: ast_op.clone(),
                        right: Box::new(right),
                    },
                    ty: result_ty,
                    is_lvalue: false,
                })
            }
            DExpr::Index { base, index } => {
                let base = self.check_expr(base)?;
                let index = self.check_expr(index)?;

                if index.ty != Type::Int {
                    return Err(SemanticError::IllegalArrayIndex { ty: index.ty });
                }

                if let Type::Array { element, .. } = &base.ty {
                    Ok(Expr {
                        ty: *element.to_owned(),
                        kind: ExprKind::Index {
                            base: Box::new(base),
                            index: Box::new(index),
                        },
                        is_lvalue: true,
                    })
                } else {
                    Err(SemanticError::IllegalIndexTarget { ty: base.ty })
                }
            }

            DExpr::Call { callee, args } => {
                let func_ident = match &**callee {
                    DExpr::Ident(ident) => ident.clone(),
                    _ => {
                        return Err(SemanticError::IllegalCalleeTarget {
                            callee: *callee.clone(),
                        });
                    }
                };

                let target_func = self
                    .global_decls
                    .funcs
                    .get(&func_ident)
                    .ok_or_else(|| SemanticError::Undefined {
                        ident: func_ident.clone(),
                    })?;

                if args.len() != target_func.params.len() {
                    return Err(SemanticError::ArgCountMismatch {
                        ident: func_ident.clone(),
                        expected: target_func.params.len(),
                        got: args.len(),
                    });
                }

                let mut ast_args = Vec::with_capacity(args.len());

                for (arg_i, (arg, param)) in args.iter().zip(&target_func.params).enumerate() {
                    let arg = self.check_expr(arg)?;
                    if arg.ty != param.ty {
                        return Err(SemanticError::ArgTypeMismatch {
                            ident: func_ident.clone(),
                            expected: param.ty.clone(),
                            position: arg_i,
                            got: arg.ty,
                        });
                    }
                    ast_args.push(arg);
                }

                Ok(Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(Expr {
                            kind: ExprKind::Var(func_ident),
                            ty: Type::Void,
                            is_lvalue: false,
                        }),
                        args: ast_args,
                    },
                    ty: target_func.return_ty.clone(),
                    is_lvalue: false,
                })
            }
        }
    }

    fn get_ident_type(&self, ident: &Ident) -> Result<Type, SemanticError> {
        if let Some(params) = self.params
            && let Some(p) = params.iter().find(|p| p.name == *ident)
        {
            Ok(p.ty.clone())
        } else if let Some(l_vars) = self.local_vars
            && let Some(ty) = l_vars.get(ident)
        {
            Ok(ty.clone())
        } else if let Some(ty) = self.global_decls.vars.get(ident) {
            Ok(ty.clone())
        } else if let Some(lit) = self.global_decls.consts.get(ident) {
            Ok(literal_type(lit))
        } else if let Some(f) = self.global_decls.funcs.get(ident) {
            Ok(f.return_ty.clone())
        } else {
            Err(SemanticError::Undefined { ident: ident.clone() })
        }
    }
}

fn literal_type(lit: &Literal) -> Type {
    match lit {
        Literal::Int(_) => Type::Int,
        Literal::Real(_) => Type::Real,
        Literal::Bool(_) => Type::Bool,
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    struct TestEnv {
        decls: GlobalDDecls,
    }

    impl TestEnv {
        fn new() -> Self {
            Self {
                decls: GlobalDDecls {
                    consts: BTreeMap::new(),
                    vars: BTreeMap::new(),
                    funcs: BTreeMap::new(),
                },
            }
        }

        fn with_var(mut self, name: &'static str, ty: Type) -> Self {
            self.decls.vars.insert(Ident::of(name), ty);
            self
        }

        fn with_const(mut self, name: &'static str, val: Literal) -> Self {
            self.decls.consts.insert(Ident::of(name), val);
            self
        }

        fn with_builtins(mut self) -> Self {
            for (key, value) in BUILTIN_FUNCS.entries() {
                self.decls.funcs.insert(
                    key.clone(),
                    FuncDDecl {
                        params: value.params.to_vec(),
                        return_ty: value.return_ty.clone(),
                        vars: value.vars.clone(),
                        block: DBlock { statements: vec![] },
                    },
                );
            }
            self
        }

        fn checker(&self) -> TypeChecker<'_> {
            TypeChecker {
                global_decls: &self.decls,
                local_vars: None,
                params: None,
                func_ident: None,
                return_ty: None,
                in_loop: false,
            }
        }
    }

    #[test]
    fn test_valid_assignment() {
        let env = TestEnv::new().with_var("x", Type::Int);
        let checker = env.checker();

        let stmt = DStatement::Assign {
            target: DExpr::Ident(Ident::of("x")),
            value: DExpr::Literal(Literal::Int(5)),
        };

        assert_eq!(
            checker.check_statement(&stmt),
            Ok(Statement::Assign {
                target: Expr {
                    kind: ExprKind::Var(Ident::of("x")),
                    ty: Type::Int,
                    is_lvalue: true,
                },
                value: Expr {
                    kind: ExprKind::Literal(Literal::Int(5)),
                    ty: Type::Int,
                    is_lvalue: false,
                },
            })
        );
    }

    #[test]
    fn test_writeln() {
        let env = TestEnv::new().with_builtins();
        let checker = env.checker();

        let stmt = DStatement::Expr(DExpr::Call {
            callee: Box::new(DExpr::Ident(Ident::of("writeln"))),
            args: vec![DExpr::Literal(Literal::Int(5))],
        });

        assert_eq!(
            checker.check_statement(&stmt),
            Ok(Statement::Expr(Expr {
                kind: ExprKind::Call {
                    callee: Box::new(Expr {
                        kind: ExprKind::Var(Ident::of("writeln")),
                        ty: Type::Void,
                        is_lvalue: false,
                    }),
                    args: vec![Expr {
                        kind: ExprKind::Literal(Literal::Int(5)),
                        ty: Type::Int,
                        is_lvalue: false,
                    }],
                },
                ty: Type::Void,
                is_lvalue: false,
            }))
        );
    }

    #[test]
    fn test_invalid_lvalue_const_assignment() {
        let env = TestEnv::new().with_const("MAX", Literal::Int(100));
        let checker = env.checker();

        let stmt = DStatement::Assign {
            target: DExpr::Ident(Ident::of("MAX")),
            value: DExpr::Literal(Literal::Int(5)),
        };

        assert!(matches!(
            checker.check_statement(&stmt),
            Err(SemanticError::NotLValue { .. })
        ));
    }

    #[test]
    fn test_type_mismatch() {
        let env = TestEnv::new().with_var("x", Type::Int);
        let checker = env.checker();

        let stmt = DStatement::Assign {
            target: DExpr::Ident(Ident::of("x")),
            value: DExpr::Literal(Literal::Bool(true)),
        };

        assert_eq!(
            checker.check_statement(&stmt),
            Err(SemanticError::AssignMismatch {
                target: Type::Int,
                value: Type::Bool,
            })
        );
    }

    #[test]
    fn test_break_continue_in_loop() {
        let env = TestEnv::new();
        let mut checker = env.checker();
        checker.in_loop = true;

        assert_eq!(checker.check_statement(&DStatement::Break), Ok(Statement::Break));
        assert_eq!(checker.check_statement(&DStatement::Continue), Ok(Statement::Continue));
    }

    #[test]
    fn test_break_continue_outside_loop() {
        let env = TestEnv::new();
        let checker = env.checker();

        assert_eq!(
            checker.check_statement(&DStatement::Break),
            Err(SemanticError::BreakOutsideLoop)
        );
        assert_eq!(
            checker.check_statement(&DStatement::Continue),
            Err(SemanticError::ContinueOutsideLoop)
        );
    }

    #[test]
    fn test_return_translation() {
        let func_ident = Ident::of("my_func");
        let return_ty = Type::Int;

        let mut env = TestEnv::new();
        env.decls.funcs.insert(
            func_ident.clone(),
            FuncDDecl {
                params: vec![],
                return_ty: return_ty.clone(),
                vars: BTreeMap::new(),
                block: DBlock { statements: vec![] },
            },
        );

        let mut checker = env.checker();
        checker.func_ident = Some(&func_ident);
        checker.return_ty = Some(&return_ty);

        let stmt = DStatement::Assign {
            target: DExpr::Ident(func_ident.clone()),
            value: DExpr::Literal(Literal::Int(5)),
        };

        assert_eq!(
            checker.check_statement(&stmt),
            Ok(Statement::Assign {
                target: Expr {
                    kind: ExprKind::Return,
                    ty: Type::Int,
                    is_lvalue: true,
                },
                value: Expr {
                    kind: ExprKind::Literal(Literal::Int(5)),
                    ty: Type::Int,
                    is_lvalue: false,
                },
            })
        );
    }

    #[test]
    fn test_recursive_call_does_not_translate_to_return() {
        let func_ident = Ident::of("my_func");
        let return_ty = Type::Int;

        let mut env = TestEnv::new();
        env.decls.funcs.insert(
            func_ident.clone(),
            FuncDDecl {
                params: vec![Param {
                    name: Ident::of("n"),
                    ty: Type::Int,
                }],
                return_ty: return_ty.clone(),
                vars: BTreeMap::new(),
                block: DBlock { statements: vec![] },
            },
        );

        let mut checker = env.checker();
        checker.func_ident = Some(&func_ident);
        checker.return_ty = Some(&return_ty);

        let expr = DExpr::Call {
            callee: Box::new(DExpr::Ident(Ident::of("my_func"))),
            args: vec![DExpr::Literal(Literal::Int(5))],
        };

        assert_eq!(
            checker.check_expr(&expr),
            Ok(Expr {
                kind: ExprKind::Call {
                    callee: Box::new(Expr {
                        kind: ExprKind::Var(Ident::of("my_func")),
                        ty: Type::Void,
                        is_lvalue: false,
                    }),
                    args: vec![Expr {
                        kind: ExprKind::Literal(Literal::Int(5)),
                        ty: Type::Int,
                        is_lvalue: false,
                    }],
                },
                ty: Type::Int,
                is_lvalue: false,
            })
        );
    }

    #[test]
    fn test_array_indexing_valid() {
        let array_ty = Type::Array {
            from: 0,
            to: 10,
            element: Box::new(Type::Int),
        };
        let env = TestEnv::new().with_var("arr", array_ty.clone());
        let checker = env.checker();

        let expr = DExpr::Index {
            base: Box::new(DExpr::Ident(Ident::of("arr"))),
            index: Box::new(DExpr::Literal(Literal::Int(3))),
        };

        assert_eq!(
            checker.check_expr(&expr),
            Ok(Expr {
                kind: ExprKind::Index {
                    base: Box::new(Expr {
                        kind: ExprKind::Var(Ident::of("arr")),
                        ty: array_ty,
                        is_lvalue: true,
                    }),
                    index: Box::new(Expr {
                        kind: ExprKind::Literal(Literal::Int(3)),
                        ty: Type::Int,
                        is_lvalue: false,
                    }),
                },
                ty: Type::Int,
                is_lvalue: true,
            })
        );
    }

    #[test]
    fn test_array_indexing_invalid_index_type() {
        let array_ty = Type::Array {
            from: 0,
            to: 10,
            element: Box::new(Type::Int),
        };
        let env = TestEnv::new().with_var("arr", array_ty);
        let checker = env.checker();

        let expr = DExpr::Index {
            base: Box::new(DExpr::Ident(Ident::of("arr"))),
            index: Box::new(DExpr::Literal(Literal::Bool(true))),
        };

        assert_eq!(
            checker.check_expr(&expr),
            Err(SemanticError::IllegalArrayIndex { ty: Type::Bool })
        );
    }

    #[test]
    fn test_array_indexing_invalid_target() {
        let env = TestEnv::new().with_var("not_arr", Type::Int);
        let checker = env.checker();

        let expr = DExpr::Index {
            base: Box::new(DExpr::Ident(Ident::of("not_arr"))),
            index: Box::new(DExpr::Literal(Literal::Int(1))),
        };

        assert_eq!(
            checker.check_expr(&expr),
            Err(SemanticError::IllegalIndexTarget { ty: Type::Int })
        );
    }

    #[test]
    fn test_if_while_invalid_condition() {
        let env = TestEnv::new();
        let checker = env.checker();

        let if_stmt = DStatement::If {
            cond: DExpr::Literal(Literal::Int(5)),
            then_branch: Box::new(DStatement::Exit),
            else_branch: None,
        };

        assert_eq!(
            checker.check_statement(&if_stmt),
            Err(SemanticError::IllegalCondition { ty: Type::Int })
        );

        let while_stmt = DStatement::While {
            cond: DExpr::Literal(Literal::Real(10)),
            body: Box::new(DStatement::Exit),
        };

        assert_eq!(
            checker.check_statement(&while_stmt),
            Err(SemanticError::IllegalCondition { ty: Type::Real })
        );
    }

    #[test]
    fn test_for_loop_type_checks() {
        let env = TestEnv::new().with_var("i", Type::Int).with_var("b", Type::Bool);
        let checker = env.checker();

        let valid_for = DStatement::For {
            name: Ident::of("i"),
            direction: DForDirection::To,
            from: DExpr::Literal(Literal::Int(1)),
            to: DExpr::Literal(Literal::Int(10)),
            body: Box::new(DStatement::Exit),
        };
        assert!(checker.check_statement(&valid_for).is_ok());

        let invalid_var_for = DStatement::For {
            name: Ident::of("b"),
            direction: DForDirection::To,
            from: DExpr::Literal(Literal::Int(1)),
            to: DExpr::Literal(Literal::Int(10)),
            body: Box::new(DStatement::Exit),
        };
        assert_eq!(
            checker.check_statement(&invalid_var_for),
            Err(SemanticError::IllegalForVar { ty: Type::Bool })
        );

        let invalid_bound_for = DStatement::For {
            name: Ident::of("i"),
            direction: DForDirection::To,
            from: DExpr::Literal(Literal::Real(1)),
            to: DExpr::Literal(Literal::Int(10)),
            body: Box::new(DStatement::Exit),
        };
        assert_eq!(
            checker.check_statement(&invalid_bound_for),
            Err(SemanticError::IllegalForFrom { ty: Type::Real })
        );
    }

    #[test]
    fn test_function_call_arg_count_mismatch() {
        let env = TestEnv::new().with_builtins();
        let checker = env.checker();

        let expr = DExpr::Call {
            callee: Box::new(DExpr::Ident(Ident::of("writeln"))),
            args: vec![DExpr::Literal(Literal::Int(5)), DExpr::Literal(Literal::Int(10))],
        };

        assert_eq!(
            checker.check_expr(&expr),
            Err(SemanticError::ArgCountMismatch {
                ident: Ident::of("writeln"),
                expected: 1,
                got: 2,
            })
        );
    }

    #[test]
    fn test_function_call_arg_type_mismatch() {
        let env = TestEnv::new().with_builtins();
        let checker = env.checker();

        let expr = DExpr::Call {
            callee: Box::new(DExpr::Ident(Ident::of("writeln"))),
            args: vec![DExpr::Literal(Literal::Bool(true))],
        };

        assert_eq!(
            checker.check_expr(&expr),
            Err(SemanticError::ArgTypeMismatch {
                ident: Ident::of("writeln"),
                expected: Type::Int,
                position: 0,
                got: Type::Bool,
            })
        );
    }

    #[test]
    fn test_binary_op_mismatch() {
        let env = TestEnv::new();
        let checker = env.checker();

        let expr = DExpr::Binary {
            left: Box::new(DExpr::Literal(Literal::Bool(true))),
            op: Op::Plus,
            right: Box::new(DExpr::Literal(Literal::Int(5))),
        };

        assert_eq!(
            checker.check_expr(&expr),
            Err(SemanticError::BinaryOpenradsTypeMismatch {
                op: Op::Plus,
                left: Type::Bool,
                right: Type::Int,
            })
        );
    }

    #[test]
    fn test_binary_op_illegal_operands() {
        let env = TestEnv::new();
        let checker = env.checker();

        let expr = DExpr::Binary {
            left: Box::new(DExpr::Literal(Literal::Bool(true))),
            op: Op::Plus,
            right: Box::new(DExpr::Literal(Literal::Bool(true))),
        };

        assert_eq!(
            checker.check_expr(&expr),
            Err(SemanticError::IllegalBinaryOperands {
                op: Op::Plus,
                operand_ty: Type::Bool,
            })
        );
    }

    #[test]
    fn test_redefinition_error_caught() {
        let result = global_idents(&[
            DDecl::Var {
                names: vec![Ident::of("x")],
                ty: DType::Int,
            },
            DDecl::Var {
                names: vec![Ident::of("x")],
                ty: DType::Real,
            },
        ]);
        assert_eq!(result, Err(SemanticError::Redefinition { ident: Ident::of("x") }));
    }
}
