use std::collections::HashMap;

use crate::{
    lexer::token::{Ident, Literal},
    semantic::{
        BUILTIN_FUNCS,
        ast::{Ast, BinaryOp, Block, Expr, ExprKind, ForDirection, Func, Param, Statement, Type, UnaryOp},
    },
    stack::Instruction,
};

/// Translates AST to stack instructions
pub fn translate(ast: Ast) -> impl Iterator<Item = Instruction> {
    Translator::new(ast).filter(|x| {
        !matches!(
            x,
            Instruction::StAlloc(0)
                | Instruction::StFree(0)
                | Instruction::StMove { by: 0, .. }
                | Instruction::StMove { bytes: 0, .. }
        )
    })
}

enum Task {
    Emit(Instruction),
    TranslateVar { name: Ident, ty: Type },
    TranslateFunc { name: Ident, func: Func },
    TranslateEntrypoint(Block),
    TranslateBlock(Block),
    TranslateSatement(Statement),
    TranslateExpr(Expr),
    TranslateAddr(Expr),
    PushLoopContext(String, String),
    PopLoopContext,
}

struct Translator {
    stack: Vec<Task>,
    return_value_offset: i64,
    return_value_size: i64,
    local_offsets: HashMap<Ident, i64>,
    label_count: usize,
    loop_ctx: Vec<(String, String)>,
    current_epilogue: String,
}

impl Translator {
    fn new(ast: Ast) -> Translator {
        let Ast {
            name: _,
            vars,
            funcs,
            entrypoint,
        } = ast;

        Translator {
            stack: {
                let mut stack = Vec::new();
                stack.push(Task::TranslateEntrypoint(entrypoint));
                for (name, func) in funcs.into_iter().rev() {
                    if !BUILTIN_FUNCS.contains_key(&name) {
                        stack.push(Task::TranslateFunc { name, func });
                    }
                }
                for (name, ty) in vars.into_iter().rev() {
                    stack.push(Task::TranslateVar { name, ty });
                }
                stack
            },
            local_offsets: HashMap::new(),
            return_value_offset: 0,
            return_value_size: 0,
            label_count: 0,
            loop_ctx: Vec::new(),
            current_epilogue: String::new(),
        }
    }

    fn new_label(&mut self, prefix: &str) -> String {
        let label = format!("{}_{}", prefix, self.label_count);
        self.label_count += 1;
        label
    }

    fn push_addr_of_ident(&mut self, ident: &Ident) {
        if let Some(&offset) = self.local_offsets.get(ident) {
            self.stack.push(Task::Emit(Instruction::PushLocalAddr(offset)));
        } else {
            self.stack
                .push(Task::Emit(Instruction::PushGlobalAddr(ident.0.to_string())));
        }
    }

    fn translate_var(&mut self, name: Ident, ty: &Type) {
        self.stack
            .push(Task::Emit(Instruction::VarDecl { name, size: ty.size() }));
    }

    fn translate_func(&mut self, name: &Ident, func: Func) {
        let Func {
            params,
            return_ty,
            vars,
            block,
        } = func;

        self.local_offsets.clear();
        self.return_value_offset = 0;
        self.return_value_size = 0;

        self.current_epilogue = format!("{}_epilogue", name.0);

        let mut param_offset = 16; // 8 for return addr + 8 for old base ptr
        for Param { name, ty } in params.iter().rev() {
            self.local_offsets.insert(name.clone(), param_offset);
            param_offset += ty.size().cast_signed();
        }
        let params_size = param_offset - 16;

        let return_size = return_ty.size().cast_signed();
        self.return_value_size = return_size;
        self.return_value_offset = -return_size;

        let mut local_offset: i64 = self.return_value_offset;

        for (name, ty) in vars {
            local_offset -= ty.size().cast_signed();
            self.local_offsets.insert(name.clone(), local_offset);
        }

        let locals_size = -local_offset - return_size;

        // Return
        self.stack.push(Task::Emit(Instruction::Return));

        // Push return addr
        self.stack.push(Task::Emit(Instruction::PopFromSpecialReg2));

        // Restore base pointer
        self.stack.push(Task::Emit(Instruction::PopBasePtr));
        self.stack.push(Task::Emit(Instruction::PopFromSpecialReg1));

        // Free parameters, return addr and base pointer, keep space for return value
        self.stack.push(Task::Emit(Instruction::StFree(params_size + 8 + 8)));

        // Move return value to beginning of params
        self.stack.push(Task::Emit(Instruction::StMove {
            from: 0,
            bytes: return_size,
            by: params_size + 8 + 8,
        }));

        // Push return addr to special register 2
        self.stack.push(Task::Emit(Instruction::PushToSpecialReg2));
        self.stack.push(Task::Emit(Instruction::Read(8)));
        self.stack.push(Task::Emit(Instruction::PushLocalAddr(8)));

        // Push old base pointer to special register 1
        self.stack.push(Task::Emit(Instruction::PushToSpecialReg1));
        self.stack.push(Task::Emit(Instruction::Read(8)));
        self.stack.push(Task::Emit(Instruction::PushLocalAddr(0)));

        // Free locals
        self.stack.push(Task::Emit(Instruction::StFree(locals_size)));

        // Emit epilogue
        self.stack
            .push(Task::Emit(Instruction::Label(self.current_epilogue.clone())));

        self.stack.push(Task::TranslateBlock(block));
        self.stack
            .push(Task::Emit(Instruction::StAlloc(return_size + locals_size)));

        self.stack.push(Task::Emit(Instruction::SetBasePtrToStackPtr));
        self.stack.push(Task::Emit(Instruction::PushBasePtr));
        self.stack.push(Task::Emit(Instruction::FuncDecl(name.0.to_string())));
    }

    fn translate_entrypoint(&mut self, block: Block) {
        self.local_offsets.clear();
        self.return_value_offset = 0;
        self.return_value_size = 0;
        self.current_epilogue = "entrypoint_epilogue".to_string();

        self.stack.push(Task::Emit(Instruction::Call("exit_ok".to_owned())));
        self.stack
            .push(Task::Emit(Instruction::Label(self.current_epilogue.clone())));
        self.stack.push(Task::TranslateBlock(block));
        self.stack.push(Task::Emit(Instruction::EntrypointDecl));
    }

    fn translate_block(&mut self, block: Block) {
        for statement in block.statements.into_iter().rev() {
            self.stack.push(Task::TranslateSatement(statement));
        }
    }

    #[allow(clippy::too_many_lines)]
    fn translate_statement(&mut self, stmt: Statement) {
        match stmt {
            Statement::Block(block) => self.stack.push(Task::TranslateBlock(block)),
            Statement::Assign { target, value } => {
                self.stack.push(Task::Emit(Instruction::Write(target.ty.size())));
                self.stack.push(Task::TranslateExpr(value));
                self.stack.push(Task::TranslateAddr(target));
            }
            Statement::Expr(expr) => {
                self.stack.push(Task::Emit(Instruction::Pop(expr.ty.size())));
                self.stack.push(Task::TranslateExpr(expr));
            }
            Statement::Exit => {
                self.stack
                    .push(Task::Emit(Instruction::Jmp(self.current_epilogue.clone())));
            }
            Statement::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let end_label = self.new_label("end_if");

                if let Some(else_branch) = else_branch {
                    let else_label = self.new_label("else");

                    self.stack.push(Task::Emit(Instruction::Label(end_label.clone())));
                    self.stack.push(Task::TranslateSatement(*else_branch));
                    self.stack.push(Task::Emit(Instruction::Label(else_label.clone())));
                    self.stack.push(Task::Emit(Instruction::Jmp(end_label.clone())));
                    self.stack.push(Task::TranslateSatement(*then_branch));
                    self.stack.push(Task::Emit(Instruction::JmpIfFalse(else_label)));
                    self.stack.push(Task::TranslateExpr(cond));
                } else {
                    self.stack.push(Task::Emit(Instruction::Label(end_label.clone())));
                    self.stack.push(Task::TranslateSatement(*then_branch));
                    self.stack.push(Task::Emit(Instruction::JmpIfFalse(end_label)));
                    self.stack.push(Task::TranslateExpr(cond));
                }
            }
            Statement::While { cond, body } => {
                let start_label = self.new_label("while_start");
                let end_label = self.new_label("while_end");

                self.stack.push(Task::Emit(Instruction::Label(end_label.clone())));
                self.stack.push(Task::PopLoopContext);
                self.stack.push(Task::Emit(Instruction::Jmp(start_label.clone())));
                self.stack.push(Task::TranslateSatement(*body));
                self.stack
                    .push(Task::PushLoopContext(start_label.clone(), end_label.clone()));
                self.stack.push(Task::Emit(Instruction::JmpIfFalse(end_label.clone())));
                self.stack.push(Task::TranslateExpr(cond));
                self.stack.push(Task::Emit(Instruction::Label(start_label)));
            }
            Statement::For {
                name,
                direction,
                from: start,
                to: end,
                body,
            } => {
                let start_label = self.new_label("for_start");
                let end_label = self.new_label("for_end");

                self.stack.push(Task::Emit(Instruction::Label(end_label.clone())));
                self.stack.push(Task::PopLoopContext);

                self.stack.push(Task::Emit(Instruction::Jmp(start_label.clone())));
                self.stack.push(Task::Emit(Instruction::Write(8)));
                self.stack.push(Task::Emit(match direction {
                    ForDirection::To => Instruction::AddInt,
                    ForDirection::Downto => Instruction::SubInt,
                }));
                self.stack.push(Task::Emit(Instruction::PushInt(1)));
                self.stack.push(Task::TranslateExpr(Expr {
                    kind: ExprKind::Var(name.clone()),
                    ty: Type::Int,
                    is_lvalue: true,
                }));
                self.push_addr_of_ident(&name);

                self.stack.push(Task::TranslateSatement(*body));
                self.stack
                    .push(Task::PushLoopContext(start_label.clone(), end_label.clone()));

                self.stack.push(Task::Emit(Instruction::JmpIfFalse(end_label.clone())));
                self.stack.push(Task::Emit(match direction {
                    ForDirection::To => Instruction::LeqInt,
                    ForDirection::Downto => Instruction::GeqInt,
                }));
                self.stack.push(Task::TranslateExpr(end));
                self.stack.push(Task::TranslateExpr(Expr {
                    kind: ExprKind::Var(name.clone()),
                    ty: Type::Int,
                    is_lvalue: true,
                }));
                self.stack.push(Task::Emit(Instruction::Label(start_label)));

                self.stack.push(Task::Emit(Instruction::Write(8)));
                self.stack.push(Task::TranslateExpr(start));
                self.push_addr_of_ident(&name);
            }
            Statement::Break => {
                let (_, end_lbl) = self.loop_ctx.last().expect("Break outside loop");
                self.stack.push(Task::Emit(Instruction::Jmp(end_lbl.clone())));
            }
            Statement::Continue => {
                let (start_lbl, _) = self.loop_ctx.last().expect("Continue outside loop");
                self.stack.push(Task::Emit(Instruction::Jmp(start_lbl.clone())));
            }
        }
    }

    fn translate_addr(&mut self, expr: Expr) {
        match expr.kind {
            ExprKind::Var(ident) => self.push_addr_of_ident(&ident),
            ExprKind::Index { base, index } => match &base.ty {
                Type::Array { from, element, .. } => {
                    self.stack.push(Task::Emit(Instruction::AddInt));
                    self.stack.push(Task::Emit(Instruction::MulInt));
                    self.stack
                        .push(Task::Emit(Instruction::PushInt(element.size().cast_signed())));
                    self.stack.push(Task::Emit(Instruction::SubInt));
                    self.stack.push(Task::Emit(Instruction::PushInt(*from)));
                    self.stack.push(Task::TranslateExpr(*index));
                    self.stack.push(Task::TranslateAddr(*base));
                }
                _ => unreachable!("Index into non array"),
            },
            ExprKind::Return => self
                .stack
                .push(Task::Emit(Instruction::PushLocalAddr(self.return_value_offset))),
            _ => unreachable!("Referencing R-Value"),
        }
    }
    fn translate_expr(&mut self, expr: Expr) {
        match expr.kind {
            ExprKind::Var(_) | ExprKind::Index { .. } => {
                self.stack.push(Task::Emit(Instruction::Read(expr.ty.size())));
                self.stack.push(Task::TranslateAddr(expr));
            }
            ExprKind::Literal(literal) => match literal {
                Literal::Int(int) => self.stack.push(Task::Emit(Instruction::PushInt(int))),
                Literal::Real(real) => self.stack.push(Task::Emit(Instruction::PushReal(i64::from_le_bytes(
                    real.to_le_bytes(),
                )))),
                Literal::Bool(bool) => self.stack.push(Task::Emit(Instruction::PushInt(i64::from(bool)))),
            },
            ExprKind::Binary { left, op, right } => {
                let instruction = match left.ty {
                    Type::Real => match op {
                        BinaryOp::Plus => Instruction::AddReal,
                        BinaryOp::Minus => Instruction::SubReal,
                        BinaryOp::Times => Instruction::MulReal,
                        BinaryOp::Div => Instruction::DivReal,
                        BinaryOp::Eq => Instruction::EqReal,
                        BinaryOp::Neq => Instruction::NeqReal,
                        BinaryOp::Lt => Instruction::LtReal,
                        BinaryOp::Leq => Instruction::LeqReal,
                        BinaryOp::Gt => Instruction::GtReal,
                        BinaryOp::Geq => Instruction::GeqReal,
                        _ => unreachable!(),
                    },
                    _ => match op {
                        BinaryOp::Plus => Instruction::AddInt,
                        BinaryOp::Minus => Instruction::SubInt,
                        BinaryOp::Times => Instruction::MulInt,
                        BinaryOp::Div => Instruction::DivInt,
                        BinaryOp::Mod => Instruction::ModInt,
                        BinaryOp::Eq => Instruction::EqInt,
                        BinaryOp::Neq => Instruction::NeqInt,
                        BinaryOp::Lt => Instruction::LtInt,
                        BinaryOp::Leq => Instruction::LeqInt,
                        BinaryOp::Gt => Instruction::GtInt,
                        BinaryOp::Geq => Instruction::GeqInt,
                        BinaryOp::And => Instruction::And,
                        BinaryOp::Or => Instruction::Or,
                        BinaryOp::Xor => Instruction::Xor,
                        BinaryOp::Imply => Instruction::Imply,
                        BinaryOp::Equiv => Instruction::Equiv,
                    },
                };
                self.stack.push(Task::Emit(instruction));
                self.stack.push(Task::TranslateExpr(*right));
                self.stack.push(Task::TranslateExpr(*left));
            }
            ExprKind::Unary { op, expr } => match op {
                UnaryOp::Minus => {
                    self.stack.push(Task::Emit(if expr.ty == Type::Real {
                        Instruction::SubReal
                    } else {
                        Instruction::SubInt
                    }));
                    self.stack.push(Task::TranslateExpr(*expr));
                    self.stack.push(Task::Emit(Instruction::PushInt(0)));
                }
                UnaryOp::Not => {
                    self.stack.push(Task::Emit(Instruction::Not));
                    self.stack.push(Task::TranslateExpr(*expr));
                }
                UnaryOp::Plus => self.stack.push(Task::TranslateExpr(*expr)),
            },
            ExprKind::Call { callee, args } => {
                if let ExprKind::Var(ident) = callee.kind {
                    self.stack.push(Task::Emit(Instruction::Call(ident.0.to_string())));
                    for arg in args.into_iter().rev() {
                        self.stack.push(Task::TranslateExpr(arg));
                    }
                }
            }
            ExprKind::Return => {
                self.stack
                    .push(Task::Emit(Instruction::Read(self.return_value_size.cast_unsigned())));
                self.stack.push(Task::TranslateAddr(expr));
            }
        }
    }
}

impl Iterator for Translator {
    type Item = Instruction;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(head) = self.stack.pop() {
            match head {
                Task::Emit(instruction) => return Some(instruction),
                Task::TranslateVar { name, ty } => self.translate_var(name, &ty),
                Task::TranslateFunc { name, func } => self.translate_func(&name, func),
                Task::TranslateEntrypoint(block) => self.translate_entrypoint(block),
                Task::TranslateBlock(block) => self.translate_block(block),
                Task::TranslateSatement(statement) => self.translate_statement(statement),
                Task::TranslateExpr(expr) => self.translate_expr(expr),
                Task::TranslateAddr(expr) => self.translate_addr(expr),
                Task::PushLoopContext(start, end) => self.loop_ctx.push((start, end)),
                Task::PopLoopContext => {
                    self.loop_ctx.pop();
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, collections::BTreeMap};

    use super::*;

    fn ident(s: &str) -> Ident {
        Ident(Cow::Owned(s.to_string()))
    }

    #[test]
    fn test_math_translation() {
        let ast = Ast {
            name: ident("test"),
            vars: BTreeMap::new(),
            funcs: BTreeMap::new(),
            entrypoint: Block {
                statements: vec![Statement::Expr(Expr {
                    kind: ExprKind::Binary {
                        left: Box::new(Expr {
                            kind: ExprKind::Literal(Literal::Int(10)),
                            ty: Type::Int,
                            is_lvalue: false,
                        }),
                        op: BinaryOp::Plus,
                        right: Box::new(Expr {
                            kind: ExprKind::Literal(Literal::Int(5)),
                            ty: Type::Int,
                            is_lvalue: false,
                        }),
                    },
                    ty: Type::Int,
                    is_lvalue: false,
                })],
            },
        };

        let mut instrs = translate(ast);
        assert_eq!(instrs.next(), Some(Instruction::EntrypointDecl));

        assert_eq!(instrs.next(), Some(Instruction::PushInt(10)));
        assert_eq!(instrs.next(), Some(Instruction::PushInt(5)));
        assert_eq!(instrs.next(), Some(Instruction::AddInt));
        assert_eq!(instrs.next(), Some(Instruction::Pop(Type::Int.size())));
        assert_eq!(
            instrs.next(),
            Some(Instruction::Label("entrypoint_epilogue".to_owned()))
        );
        assert_eq!(instrs.next(), Some(Instruction::Call("exit_ok".to_owned())));
        assert_eq!(instrs.next(), None);
    }

    #[test]
    fn test_if_translation() {
        let ast = Ast {
            name: ident("test_if"),
            vars: BTreeMap::new(),
            funcs: BTreeMap::new(),
            entrypoint: Block {
                statements: vec![Statement::If {
                    cond: Expr {
                        kind: ExprKind::Literal(Literal::Bool(true)),
                        ty: Type::Bool,
                        is_lvalue: false,
                    },
                    then_branch: Box::new(Statement::Expr(Expr {
                        kind: ExprKind::Literal(Literal::Int(1)),
                        ty: Type::Int,
                        is_lvalue: false,
                    })),
                    else_branch: Some(Box::new(Statement::Expr(Expr {
                        kind: ExprKind::Literal(Literal::Int(0)),
                        ty: Type::Int,
                        is_lvalue: false,
                    }))),
                }],
            },
        };

        let mut instrs = translate(ast);
        assert_eq!(instrs.next(), Some(Instruction::EntrypointDecl));
        assert_eq!(instrs.next(), Some(Instruction::PushInt(1)));
        assert_eq!(instrs.next(), Some(Instruction::JmpIfFalse("else_1".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::PushInt(1)));
        assert_eq!(instrs.next(), Some(Instruction::Pop(Type::Int.size())));
        assert_eq!(instrs.next(), Some(Instruction::Jmp("end_if_0".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::Label("else_1".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::PushInt(0)));
        assert_eq!(instrs.next(), Some(Instruction::Pop(Type::Int.size())));
        assert_eq!(instrs.next(), Some(Instruction::Label("end_if_0".to_owned())));
        assert_eq!(
            instrs.next(),
            Some(Instruction::Label("entrypoint_epilogue".to_owned()))
        );
        assert_eq!(instrs.next(), Some(Instruction::Call("exit_ok".to_owned())));
        assert_eq!(instrs.next(), None);
    }

    #[test]
    fn test_offset_calculation() {
        let func = Func {
            params: Cow::Owned(vec![Param {
                name: ident("param1"),
                ty: Type::Int,
            }]),
            return_ty: Type::Int,
            vars: BTreeMap::from([(ident("loc1"), Type::Int)]),
            block: Block { statements: vec![] },
        };

        let ast = Ast {
            name: ident("test_offset"),
            vars: BTreeMap::new(),
            funcs: BTreeMap::from([(ident("test_func"), func)]),
            entrypoint: Block { statements: vec![] },
        };

        let mut instrs = translate(ast);
        assert_eq!(instrs.next(), Some(Instruction::FuncDecl("test_func".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::PushBasePtr));
        assert_eq!(instrs.next(), Some(Instruction::SetBasePtrToStackPtr));
        assert_eq!(instrs.next(), Some(Instruction::StAlloc(16)));
        assert_eq!(instrs.next(), Some(Instruction::Label("test_func_epilogue".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::StFree(8)));
        assert_eq!(instrs.next(), Some(Instruction::PushLocalAddr(0)));
        assert_eq!(instrs.next(), Some(Instruction::Read(8)));
        assert_eq!(instrs.next(), Some(Instruction::PushToSpecialReg1));
        assert_eq!(instrs.next(), Some(Instruction::PushLocalAddr(8)));
        assert_eq!(instrs.next(), Some(Instruction::Read(8)));
        assert_eq!(instrs.next(), Some(Instruction::PushToSpecialReg2));
        assert_eq!(
            instrs.next(),
            Some(Instruction::StMove {
                from: 0,
                bytes: 8,
                by: 24
            })
        );
        assert_eq!(instrs.next(), Some(Instruction::StFree(24)));
        assert_eq!(instrs.next(), Some(Instruction::PopFromSpecialReg1));
        assert_eq!(instrs.next(), Some(Instruction::PopBasePtr));
        assert_eq!(instrs.next(), Some(Instruction::PopFromSpecialReg2));
        assert_eq!(instrs.next(), Some(Instruction::Return));
        assert_eq!(instrs.next(), Some(Instruction::EntrypointDecl));
        assert_eq!(
            instrs.next(),
            Some(Instruction::Label("entrypoint_epilogue".to_owned()))
        );
        assert_eq!(instrs.next(), Some(Instruction::Call("exit_ok".to_owned())));
        assert_eq!(instrs.next(), None);
    }

    #[test]
    fn test_nested_loops_and_context() {
        // Tests while containing a for-loop containing a break.
        // Verifies that Break correctly attaches to the inner for-loop's context
        // and doesn't prematurely kill the outer while-loop.
        let ast = Ast {
            name: ident("test_loops"),
            vars: BTreeMap::new(),
            funcs: BTreeMap::new(),
            entrypoint: Block {
                statements: vec![Statement::While {
                    cond: Expr {
                        kind: ExprKind::Literal(Literal::Bool(true)),
                        ty: Type::Bool,
                        is_lvalue: false,
                    },
                    body: Box::new(Statement::Block(Block {
                        statements: vec![
                            Statement::For {
                                name: ident("i"),
                                direction: ForDirection::To,
                                from: Expr {
                                    kind: ExprKind::Literal(Literal::Int(0)),
                                    ty: Type::Int,
                                    is_lvalue: false,
                                },
                                to: Expr {
                                    kind: ExprKind::Literal(Literal::Int(10)),
                                    ty: Type::Int,
                                    is_lvalue: false,
                                },
                                body: Box::new(Statement::Break), // Inner break
                            },
                            Statement::Continue, // Outer continue
                        ],
                    })),
                }],
            },
        };

        let mut instrs = translate(ast);
        assert_eq!(instrs.next(), Some(Instruction::EntrypointDecl));

        // While loop setup
        assert_eq!(instrs.next(), Some(Instruction::Label("while_start_0".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::PushInt(1)));
        assert_eq!(instrs.next(), Some(Instruction::JmpIfFalse("while_end_1".to_owned())));

        // For initialization (i := 0)
        assert_eq!(instrs.next(), Some(Instruction::PushGlobalAddr("i".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::PushInt(0)));
        assert_eq!(instrs.next(), Some(Instruction::Write(8)));

        // For condition (i <= 10)
        assert_eq!(instrs.next(), Some(Instruction::Label("for_start_2".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::PushGlobalAddr("i".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::Read(8)));
        assert_eq!(instrs.next(), Some(Instruction::PushInt(10)));
        assert_eq!(instrs.next(), Some(Instruction::LeqInt));
        assert_eq!(instrs.next(), Some(Instruction::JmpIfFalse("for_end_3".to_owned())));

        // For body: Inner Break
        // Verify it jumps to the FOR loop's end, NOT the WHILE loop's end.
        assert_eq!(instrs.next(), Some(Instruction::Jmp("for_end_3".to_owned())));

        // For update (i += 1)
        assert_eq!(instrs.next(), Some(Instruction::PushGlobalAddr("i".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::PushGlobalAddr("i".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::Read(8)));
        assert_eq!(instrs.next(), Some(Instruction::PushInt(1)));
        assert_eq!(instrs.next(), Some(Instruction::AddInt));
        assert_eq!(instrs.next(), Some(Instruction::Write(8)));
        assert_eq!(instrs.next(), Some(Instruction::Jmp("for_start_2".to_owned())));

        // For loop termination
        assert_eq!(instrs.next(), Some(Instruction::Label("for_end_3".to_owned())));

        // While body: Outer Continue
        assert_eq!(instrs.next(), Some(Instruction::Jmp("while_start_0".to_owned())));

        // Auto While body restart
        assert_eq!(instrs.next(), Some(Instruction::Jmp("while_start_0".to_owned())));

        // While loop termination
        assert_eq!(instrs.next(), Some(Instruction::Label("while_end_1".to_owned())));

        // Program Epilogue
        assert_eq!(
            instrs.next(),
            Some(Instruction::Label("entrypoint_epilogue".to_owned()))
        );
        assert_eq!(instrs.next(), Some(Instruction::Call("exit_ok".to_owned())));
        assert_eq!(instrs.next(), None);
    }

    #[test]
    fn test_complex_array_indexing_math() {
        // Tests: arr[5] := 10
        // Verifies that the order of Push/Sub/Mul guarantees correct index calculation constraints.
        let array_type = Type::Array {
            from: 1,
            to: 10,
            element: Box::new(Type::Int),
        };

        let ast = Ast {
            name: ident("test_array"),
            vars: BTreeMap::new(), // assumed global arr for test isolation
            funcs: BTreeMap::new(),
            entrypoint: Block {
                statements: vec![Statement::Assign {
                    target: Expr {
                        kind: ExprKind::Index {
                            base: Box::new(Expr {
                                kind: ExprKind::Var(ident("arr")),
                                ty: array_type.clone(),
                                is_lvalue: true,
                            }),
                            index: Box::new(Expr {
                                kind: ExprKind::Literal(Literal::Int(5)),
                                ty: Type::Int,
                                is_lvalue: false,
                            }),
                        },
                        ty: Type::Int,
                        is_lvalue: true,
                    },
                    value: Expr {
                        kind: ExprKind::Literal(Literal::Int(10)),
                        ty: Type::Int,
                        is_lvalue: false,
                    },
                }],
            },
        };

        let mut instrs = translate(ast);
        assert_eq!(instrs.next(), Some(Instruction::EntrypointDecl));

        // Resolve LHS target address mapping logic
        assert_eq!(instrs.next(), Some(Instruction::PushGlobalAddr("arr".to_owned())));
        assert_eq!(instrs.next(), Some(Instruction::PushInt(5))); // index evaluates to 5
        assert_eq!(instrs.next(), Some(Instruction::PushInt(1))); // array lower bound
        assert_eq!(instrs.next(), Some(Instruction::SubInt)); // Pops 1, Pops 5 -> 4
        assert_eq!(instrs.next(), Some(Instruction::PushInt(8))); // internal element width
        assert_eq!(instrs.next(), Some(Instruction::MulInt)); // 4 * 8 -> 32
        assert_eq!(instrs.next(), Some(Instruction::AddInt)); // base + 32 byte offset

        // Resolve RHS Value
        assert_eq!(instrs.next(), Some(Instruction::PushInt(10)));

        // Emit Memory Write
        assert_eq!(instrs.next(), Some(Instruction::Write(8)));

        // Program Epilogue
        assert_eq!(
            instrs.next(),
            Some(Instruction::Label("entrypoint_epilogue".to_owned()))
        );
        assert_eq!(instrs.next(), Some(Instruction::Call("exit_ok".to_owned())));
        assert_eq!(instrs.next(), None);
    }
}
