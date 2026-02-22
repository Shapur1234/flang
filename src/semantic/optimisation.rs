use crate::{
    lexer::token::Literal,
    semantic::ast::{Ast, BinaryOp, Block, ExprKind, Statement, UnaryOp},
};

/// Optimizes the AST
///
/// Removes redundant blocks, folds constant expressions, and eliminates dead code after `break`/`continue`/`exit`
pub fn optimise(ast: Ast) -> Ast {
    let mut ast = ast;

    for func in ast.funcs.values_mut() {
        optimize_block(&mut func.block);
    }
    optimize_block(&mut ast.entrypoint);

    ast
}

fn optimize_block(block: &mut Block) {
    let old_statements = std::mem::take(&mut block.statements);
    block.statements.reserve(old_statements.len());

    for mut statement in old_statements {
        optimize_statement(&mut statement);

        let end_block = matches!(statement, Statement::Break | Statement::Continue | Statement::Exit);

        if let Statement::Block(mut inner_block) = statement {
            block.statements.append(&mut inner_block.statements);
        } else {
            block.statements.push(statement);
        }

        if end_block {
            break;
        }
    }
}

fn optimize_statement(statement: &mut Statement) {
    match statement {
        Statement::Block(block) => {
            optimize_block(block);
            if block.statements.len() == 1 {
                *statement = block.statements.pop().unwrap();
            }
        }
        Statement::Assign { target, value } => {
            optimise_expr(&mut target.kind);
            optimise_expr(&mut value.kind);

            if target == value {
                *statement = Statement::Block(Block { statements: vec![] });
            }
        }
        Statement::Expr(expr) => optimise_expr(&mut expr.kind),
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            optimise_expr(&mut cond.kind);
            optimize_statement(then_branch);
            if let Some(else_stmt) = else_branch {
                optimize_statement(else_stmt);
            }

            match cond.kind {
                ExprKind::Literal(Literal::Bool(true)) => {
                    *statement = *then_branch.clone();
                }
                ExprKind::Literal(Literal::Bool(false)) => match else_branch {
                    Some(else_stmt) => {
                        *statement = *else_stmt.clone();
                    }
                    None => {
                        *statement = Statement::Block(Block { statements: vec![] });
                    }
                },
                _ => (),
            }
        }
        Statement::While { cond, body } => {
            optimise_expr(&mut cond.kind);
            optimize_statement(body);

            if let ExprKind::Literal(Literal::Bool(false)) = cond.kind {
                *statement = Statement::Block(Block { statements: vec![] });
            }
        }
        Statement::For {
            from: start,
            to: end,
            body,
            ..
        } => {
            optimise_expr(&mut start.kind);
            optimise_expr(&mut end.kind);
            optimize_statement(body);
        }
        _ => {}
    }
}

#[allow(
    clippy::too_many_lines,
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]
fn optimise_expr(expr: &mut ExprKind) {
    match expr {
        ExprKind::Binary { op, left, right } => {
            optimise_expr(&mut left.kind);
            optimise_expr(&mut right.kind);

            let replacement = match (&mut left.kind, &mut right.kind) {
                (ExprKind::Literal(left_lit), ExprKind::Literal(right_lit)) => match (op, left_lit, right_lit) {
                    (BinaryOp::Plus, Literal::Int(left), Literal::Int(right)) => {
                        Some(Literal::Int(left.wrapping_add(*right)))
                    }
                    (BinaryOp::Minus, Literal::Int(left), Literal::Int(right)) => {
                        Some(Literal::Int(left.wrapping_sub(*right)))
                    }
                    (BinaryOp::Times, Literal::Int(left), Literal::Int(right)) => {
                        Some(Literal::Int(left.wrapping_mul(*right)))
                    }
                    (BinaryOp::Div, Literal::Int(left), Literal::Int(right)) => {
                        Some(Literal::Int(if *right == 0 { 0 } else { left.wrapping_div(*right) }))
                    }
                    (BinaryOp::Mod, Literal::Int(left), Literal::Int(right)) => {
                        Some(Literal::Int(if *right == 0 { 0 } else { left.wrapping_rem(*right) }))
                    }
                    (BinaryOp::Plus, Literal::Real(left), Literal::Real(right)) => Some(Literal::real(
                        Literal::real_to_f64(*left) + Literal::real_to_f64(*right),
                    )),
                    (BinaryOp::Minus, Literal::Real(left), Literal::Real(right)) => Some(Literal::real(
                        Literal::real_to_f64(*left) - Literal::real_to_f64(*right),
                    )),
                    (BinaryOp::Times, Literal::Real(left), Literal::Real(right)) => Some(Literal::real(
                        Literal::real_to_f64(*left) * Literal::real_to_f64(*right),
                    )),
                    (BinaryOp::Div, Literal::Real(left), Literal::Real(right)) => Some(Literal::real(
                        Literal::real_to_f64(*left) / Literal::real_to_f64(*right),
                    )),
                    (BinaryOp::And, Literal::Int(left), Literal::Int(right)) => Some(Literal::Int(*left & *right)),
                    (BinaryOp::Or, Literal::Int(left), Literal::Int(right)) => Some(Literal::Int(*left | *right)),
                    (BinaryOp::Xor, Literal::Int(left), Literal::Int(right)) => Some(Literal::Int(*left ^ *right)),
                    (BinaryOp::Imply, Literal::Int(left), Literal::Int(right)) => Some(Literal::Int(!*left | *right)),
                    (BinaryOp::Equiv, Literal::Int(left), Literal::Int(right)) => Some(Literal::Int(!(*left ^ *right))),
                    (BinaryOp::And, Literal::Bool(left), Literal::Bool(right)) => Some(Literal::Bool(*left && *right)),
                    (BinaryOp::Or, Literal::Bool(left), Literal::Bool(right)) => Some(Literal::Bool(*left || *right)),
                    (BinaryOp::Xor, Literal::Bool(left), Literal::Bool(right)) => Some(Literal::Bool(*left ^ *right)),
                    (BinaryOp::Imply, Literal::Bool(left), Literal::Bool(right)) => {
                        Some(Literal::Bool(!*left || *right))
                    }
                    (BinaryOp::Equiv | BinaryOp::Eq, Literal::Bool(left), Literal::Bool(right)) => {
                        Some(Literal::Bool(left == right))
                    }
                    (BinaryOp::Neq, Literal::Int(left), Literal::Int(right)) => Some(Literal::Bool(left != right)),
                    (BinaryOp::Lt, Literal::Int(left), Literal::Int(right)) => Some(Literal::Bool(left < right)),
                    (BinaryOp::Gt, Literal::Int(left), Literal::Int(right)) => Some(Literal::Bool(left > right)),
                    (BinaryOp::Leq, Literal::Int(left), Literal::Int(right)) => Some(Literal::Bool(left <= right)),
                    (BinaryOp::Geq, Literal::Int(left), Literal::Int(right)) => Some(Literal::Bool(left >= right)),
                    (BinaryOp::Eq, Literal::Real(left), Literal::Real(right)) => Some(Literal::Bool(
                        Literal::real_to_f64(*left) == Literal::real_to_f64(*right),
                    )),
                    (BinaryOp::Neq, Literal::Real(left), Literal::Real(right)) => Some(Literal::Bool(
                        Literal::real_to_f64(*left) != Literal::real_to_f64(*right),
                    )),
                    (BinaryOp::Lt, Literal::Real(left), Literal::Real(right)) => Some(Literal::Bool(
                        Literal::real_to_f64(*left) < Literal::real_to_f64(*right),
                    )),
                    (BinaryOp::Gt, Literal::Real(left), Literal::Real(right)) => Some(Literal::Bool(
                        Literal::real_to_f64(*left) > Literal::real_to_f64(*right),
                    )),
                    (BinaryOp::Leq, Literal::Real(left), Literal::Real(right)) => Some(Literal::Bool(
                        Literal::real_to_f64(*left) <= Literal::real_to_f64(*right),
                    )),
                    (BinaryOp::Geq, Literal::Real(left), Literal::Real(right)) => Some(Literal::Bool(
                        Literal::real_to_f64(*left) >= Literal::real_to_f64(*right),
                    )),
                    (BinaryOp::Neq, Literal::Bool(left), Literal::Bool(right)) => Some(Literal::Bool(left != right)),
                    _ => None,
                }
                .map(ExprKind::Literal),
                _ => match (op, &left.kind, &right.kind) {
                    (BinaryOp::Plus | BinaryOp::Minus, e, ExprKind::Literal(Literal::Int(0)))
                    | (BinaryOp::Plus, ExprKind::Literal(Literal::Int(0)), e)
                    | (BinaryOp::Times | BinaryOp::Div, e, ExprKind::Literal(Literal::Int(1)))
                    | (BinaryOp::Times, ExprKind::Literal(Literal::Int(1)), e)
                    | (BinaryOp::And, e, ExprKind::Literal(Literal::Bool(true)))
                    | (BinaryOp::And, ExprKind::Literal(Literal::Bool(true)), e)
                    | (BinaryOp::Or, e, ExprKind::Literal(Literal::Bool(false)))
                    | (BinaryOp::Or, ExprKind::Literal(Literal::Bool(false)), e) => Some(e.clone()),

                    (BinaryOp::Times, _, ExprKind::Literal(Literal::Int(0)))
                    | (BinaryOp::Times, ExprKind::Literal(Literal::Int(0)), _) => {
                        Some(ExprKind::Literal(Literal::Int(0)))
                    }

                    (BinaryOp::Plus, e, ExprKind::Literal(Literal::Real(v)))
                    | (BinaryOp::Plus, ExprKind::Literal(Literal::Real(v)), e)
                        if Literal::real_to_f64(*v) == 0.0 =>
                    {
                        Some(e.clone())
                    }

                    (BinaryOp::Minus, e, ExprKind::Literal(Literal::Real(v))) if Literal::real_to_f64(*v) == 0.0 => {
                        Some(e.clone())
                    }

                    (BinaryOp::Times | BinaryOp::Div, e, ExprKind::Literal(Literal::Real(v)))
                    | (BinaryOp::Times, ExprKind::Literal(Literal::Real(v)), e)
                        if Literal::real_to_f64(*v) == 1.0 =>
                    {
                        Some(e.clone())
                    }

                    (BinaryOp::Times, _, ExprKind::Literal(Literal::Real(v)))
                    | (BinaryOp::Times, ExprKind::Literal(Literal::Real(v)), _)
                        if Literal::real_to_f64(*v) == 0.0 =>
                    {
                        Some(ExprKind::Literal(Literal::real(0.0)))
                    }

                    (BinaryOp::And, _, ExprKind::Literal(Literal::Bool(false)))
                    | (BinaryOp::And, ExprKind::Literal(Literal::Bool(false)), _) => {
                        Some(ExprKind::Literal(Literal::Bool(false)))
                    }

                    (BinaryOp::Or, _, ExprKind::Literal(Literal::Bool(true)))
                    | (BinaryOp::Or, ExprKind::Literal(Literal::Bool(true)), _) => {
                        Some(ExprKind::Literal(Literal::Bool(true)))
                    }

                    _ => None,
                },
            };

            if let Some(replacement) = replacement {
                *expr = replacement;
            }
        }
        ExprKind::Unary { op, expr: inner_expr } => {
            optimise_expr(&mut inner_expr.kind);

            let replacement = match &inner_expr.kind {
                ExprKind::Literal(lit) => match (op, lit) {
                    (UnaryOp::Minus, Literal::Int(value)) => Some(Literal::Int(value.wrapping_neg())),
                    (UnaryOp::Plus, Literal::Int(value)) => Some(Literal::Int(*value)),
                    (UnaryOp::Minus, Literal::Real(value)) => Some(Literal::real(-Literal::real_to_f64(*value))),
                    (UnaryOp::Plus, Literal::Real(value)) => Some(Literal::Real(*value)),
                    (UnaryOp::Not, Literal::Bool(value)) => Some(Literal::Bool(!value)),
                    (UnaryOp::Not, Literal::Int(value)) => Some(Literal::Int(!value)),
                    _ => None,
                }
                .map(ExprKind::Literal),
                _ => match (op, &inner_expr.kind) {
                    (
                        UnaryOp::Not,
                        ExprKind::Unary {
                            op: UnaryOp::Not,
                            expr: double_inner,
                        },
                    )
                    | (
                        UnaryOp::Minus,
                        ExprKind::Unary {
                            op: UnaryOp::Minus,
                            expr: double_inner,
                        },
                    ) => Some(double_inner.kind.clone()),
                    (UnaryOp::Plus, inner) => Some(inner.clone()),
                    _ => None,
                },
            };

            if let Some(replacement) = replacement {
                *expr = replacement;
            }
        }
        ExprKind::Call { callee, args } => {
            optimise_expr(&mut callee.kind);
            for arg in args.iter_mut() {
                optimise_expr(&mut arg.kind);
            }

            if let (ExprKind::Var(name), 1) = (&callee.kind, args.len())
                && let ExprKind::Literal(lit) = &args[0].kind
            {
                let folded = match (name.0.as_ref(), lit) {
                    ("bool_to_int", Literal::Bool(value)) => Some(Literal::Int(i64::from(*value))),
                    ("bool_to_real", Literal::Bool(value)) => Some(Literal::real(if *value { 1.0 } else { 0.0 })),
                    ("int_to_bool", Literal::Int(value)) => Some(Literal::Bool(*value != 0)),
                    ("int_to_real", Literal::Int(value)) => Some(Literal::real(*value as f64)),
                    ("real_to_bool", Literal::Real(value)) => Some(Literal::Bool(Literal::real_to_f64(*value) != 0.0)),
                    ("real_to_int", Literal::Real(value)) => Some(Literal::Int(Literal::real_to_f64(*value) as i64)),
                    _ => None,
                };

                if let Some(literal) = folded {
                    *expr = ExprKind::Literal(literal);
                }
            }
        }
        ExprKind::Index { base, index } => {
            optimise_expr(&mut base.kind);
            optimise_expr(&mut index.kind);
        }
        _ => {}
    }
}

#[cfg(test)]
mod fold_block_tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{
        lexer::token::Ident,
        semantic::ast::{Expr, Type},
    };

    fn create_test_ast(entrypoint: Block) -> Ast {
        Ast {
            name: Ident::of("test_prog"),
            vars: BTreeMap::new(),
            funcs: BTreeMap::new(),
            entrypoint,
        }
    }

    #[test]
    fn test_fold_single_statement_block() {
        // { { x := 5 } } -> x := 5
        let inner_stmt = Statement::Assign {
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
        };

        let nested_block = Statement::Block(Block {
            statements: vec![inner_stmt.clone()],
        });

        let ast = create_test_ast(Block {
            statements: vec![nested_block],
        });

        let optimized = optimise(ast);

        assert_eq!(optimized.entrypoint.statements.len(), 1);
        assert_eq!(optimized.entrypoint.statements[0], inner_stmt);
    }

    #[test]
    fn test_flatten_multiple_statements() {
        // { x := 1; { y := 2; z := 3 } } -> x := 1; y := 2; z := 3
        let stmt1 = Statement::Assign {
            target: Expr {
                kind: ExprKind::Var(Ident::of("x")),
                ty: Type::Int,
                is_lvalue: true,
            },
            value: Expr {
                kind: ExprKind::Literal(Literal::Int(1)),
                ty: Type::Int,
                is_lvalue: false,
            },
        };
        let stmt2 = Statement::Assign {
            target: Expr {
                kind: ExprKind::Var(Ident::of("y")),
                is_lvalue: true,
                ty: Type::Int,
            },
            value: Expr {
                kind: ExprKind::Literal(Literal::Int(2)),
                ty: Type::Int,
                is_lvalue: false,
            },
        };
        let stmt3 = Statement::Assign {
            target: Expr {
                kind: ExprKind::Var(Ident::of("z")),
                ty: Type::Int,
                is_lvalue: true,
            },
            value: Expr {
                kind: ExprKind::Literal(Literal::Int(3)),
                ty: Type::Int,
                is_lvalue: false,
            },
        };

        let inner_block = Statement::Block(Block {
            statements: vec![stmt2.clone(), stmt3.clone()],
        });

        let ast = create_test_ast(Block {
            statements: vec![stmt1.clone(), inner_block],
        });

        let optimized = optimise(ast);

        assert_eq!(optimized.entrypoint.statements.len(), 3);
        assert_eq!(optimized.entrypoint.statements[0], stmt1);
        assert_eq!(optimized.entrypoint.statements[1], stmt2);
        assert_eq!(optimized.entrypoint.statements[2], stmt3);
    }

    #[test]
    fn test_optimization_inside_control_flow() {
        // while true do { { exit } } -> while true do exit
        let inner_exit = Statement::Exit;
        let nested_block = Statement::Block(Block {
            statements: vec![inner_exit.clone()],
        });

        let while_stmt = Statement::While {
            cond: Expr {
                kind: ExprKind::Literal(Literal::Bool(true)),
                ty: Type::Bool,
                is_lvalue: false,
            },
            body: Box::new(nested_block),
        };

        let ast = create_test_ast(Block {
            statements: vec![while_stmt],
        });

        let optimized = optimise(ast);

        if let Statement::While { body, .. } = &optimized.entrypoint.statements[0] {
            assert_eq!(**body, inner_exit);
        } else {
            panic!("Expected while statement");
        }
    }
}

#[cfg(test)]
mod fold_expr_tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{
        lexer::token::Ident,
        semantic::ast::{Expr, Type},
    };

    fn e_int(val: i64) -> Expr {
        Expr {
            kind: ExprKind::Literal(Literal::Int(val)),
            ty: Type::Int,
            is_lvalue: false,
        }
    }
    fn e_bool(val: bool) -> Expr {
        Expr {
            kind: ExprKind::Literal(Literal::Bool(val)),
            ty: Type::Bool,
            is_lvalue: false,
        }
    }
    fn e_real(val: f64) -> Expr {
        Expr {
            kind: ExprKind::Literal(Literal::real(val)),
            ty: Type::Real,
            is_lvalue: false,
        }
    }
    fn e_var(name: &'static str, ty: Type) -> Expr {
        Expr {
            kind: ExprKind::Var(Ident::of(name)),
            ty,
            is_lvalue: true,
        }
    }

    fn setup_ast(stmt: Statement) -> Ast {
        Ast {
            name: Ident::of("test"),
            vars: BTreeMap::new(),
            funcs: BTreeMap::new(),
            entrypoint: Block { statements: vec![stmt] },
        }
    }

    #[test]
    fn test_fold_addition() {
        let expr = Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Plus,
                left: Box::new(e_int(2)),
                right: Box::new(e_int(3)),
            },
            ty: Type::Int,
            is_lvalue: false,
        };
        let stmt = Statement::Assign {
            target: e_var("x", Type::Int),
            value: expr,
        };

        let optimized = optimise(setup_ast(stmt));

        if let Statement::Assign { value, .. } = &optimized.entrypoint.statements[0] {
            assert_eq!(value.kind, ExprKind::Literal(Literal::Int(5)));
        } else {
            panic!("Expected assignment");
        }
    }

    #[test]
    fn test_fold_complex_logic() {
        // (1 < 2) and true  ->  true
        let comparison = Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Lt,
                left: Box::new(e_int(1)),
                right: Box::new(e_int(2)),
            },
            ty: Type::Bool,
            is_lvalue: false,
        };
        let logic = Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::And,
                left: Box::new(comparison),
                right: Box::new(e_bool(true)),
            },
            ty: Type::Bool,
            is_lvalue: false,
        };
        let stmt = Statement::Expr(logic);

        let optimized = optimise(setup_ast(stmt));

        assert_eq!(optimized.entrypoint.statements[0], Statement::Expr(e_bool(true)));
    }

    #[test]
    fn test_nested_block_and_fold_combined() {
        // { { x := 10 * 10 } }  ->  x := 100
        let inner_expr = Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Times,
                left: Box::new(e_int(10)),
                right: Box::new(e_int(10)),
            },
            ty: Type::Int,
            is_lvalue: false,
        };
        let inner_stmt = Statement::Assign {
            target: e_var("x", Type::Int),
            value: inner_expr,
        };
        let nested = Statement::Block(Block {
            statements: vec![Statement::Block(Block {
                statements: vec![inner_stmt],
            })],
        });

        let optimized = optimise(setup_ast(nested));

        assert_eq!(optimized.entrypoint.statements.len(), 1);
        if let Statement::Assign { value, .. } = &optimized.entrypoint.statements[0] {
            assert_eq!(value.kind, ExprKind::Literal(Literal::Int(100)));
        } else {
            panic!("Failed to both fold and unwrap blocks");
        }
    }

    #[test]
    fn test_bitwise_folding() {
        // 5 and 3 -> 1
        let mut expr = ExprKind::Binary {
            op: BinaryOp::And,
            left: Box::new(e_int(5)),
            right: Box::new(e_int(3)),
        };
        optimise_expr(&mut expr);
        assert_eq!(expr, ExprKind::Literal(Literal::Int(1)));

        // not 0 (bitwise) -> -1
        let mut expr_not = ExprKind::Unary {
            op: UnaryOp::Not,
            expr: Box::new(e_int(0)),
        };
        optimise_expr(&mut expr_not);
        assert_eq!(expr_not, ExprKind::Literal(Literal::Int(-1)));
    }

    #[test]
    fn test_comparison_folding() {
        // 10 >= 5 -> true
        let mut expr = ExprKind::Binary {
            op: BinaryOp::Geq,
            left: Box::new(e_int(10)),
            right: Box::new(e_int(5)),
        };
        optimise_expr(&mut expr);
        assert_eq!(expr, ExprKind::Literal(Literal::Bool(true)));

        // 10 != 10 -> false
        let mut expr_neq = ExprKind::Binary {
            op: BinaryOp::Neq,
            left: Box::new(e_int(10)),
            right: Box::new(e_int(10)),
        };
        optimise_expr(&mut expr_neq);
        assert_eq!(expr_neq, ExprKind::Literal(Literal::Bool(false)));
    }

    #[test]
    fn test_logic_implication() {
        // true imply false -> false
        let mut expr = ExprKind::Binary {
            op: BinaryOp::Imply,
            left: Box::new(e_bool(true)),
            right: Box::new(e_bool(false)),
        };
        optimise_expr(&mut expr);
        assert_eq!(expr, ExprKind::Literal(Literal::Bool(false)));
    }

    #[test]
    fn test_fold_conversion() {
        // bool_to_int(true) -> 1
        let mut expr = ExprKind::Call {
            callee: Box::new(e_var("bool_to_int", Type::Int)),
            args: vec![e_bool(true)],
        };
        optimise_expr(&mut expr);
        assert_eq!(expr, ExprKind::Literal(Literal::Int(1)));
    }

    #[test]
    fn test_fold_real_arithmetic() {
        // 1.5 + 2.5 -> 4.0
        let mut expr = ExprKind::Binary {
            op: BinaryOp::Plus,
            left: Box::new(e_real(1.5)),
            right: Box::new(e_real(2.5)),
        };
        optimise_expr(&mut expr);
        assert_eq!(expr, ExprKind::Literal(Literal::real(4.0)));
    }

    #[test]
    fn test_fold_real_comparisons() {
        // 1.5 < 2.5 -> true
        let mut expr = ExprKind::Binary {
            op: BinaryOp::Lt,
            left: Box::new(e_real(1.5)),
            right: Box::new(e_real(2.5)),
        };
        optimise_expr(&mut expr);
        assert_eq!(expr, ExprKind::Literal(Literal::Bool(true)));
    }

    #[test]
    fn test_fold_real_unary() {
        // -5.5 -> -5.5
        let mut expr = ExprKind::Unary {
            op: UnaryOp::Minus,
            expr: Box::new(e_real(5.5)),
        };
        optimise_expr(&mut expr);
        assert_eq!(expr, ExprKind::Literal(Literal::real(-5.5)));
    }

    #[test]
    fn test_remove_self_assignment() {
        let stmt = Statement::Assign {
            target: e_var("I", Type::Int),
            value: e_var("I", Type::Int),
        };

        let optimized = optimise(setup_ast(Statement::Block(Block { statements: vec![stmt] })));
        assert!(optimized.entrypoint.statements.is_empty());
    }

    #[test]
    fn test_identity_folding() {
        // I * 1 -> I
        let mut expr = ExprKind::Binary {
            op: BinaryOp::Times,
            left: Box::new(e_var("I", Type::Int)),
            right: Box::new(e_int(1)),
        };
        optimise_expr(&mut expr);
        assert_eq!(expr, ExprKind::Var(Ident::of("I")));

        // b and true -> b
        let mut expr_bool = ExprKind::Binary {
            op: BinaryOp::And,
            left: Box::new(e_bool(true)),
            right: Box::new(e_var("b", Type::Bool)),
        };
        optimise_expr(&mut expr_bool);
        assert_eq!(expr_bool, ExprKind::Var(Ident::of("b")));

        // -(-x) -> x
        let mut expr_unary = ExprKind::Unary {
            op: UnaryOp::Minus,
            expr: Box::new(Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Minus,
                    expr: Box::new(e_var("x", Type::Int)),
                },
                ty: Type::Int,
                is_lvalue: false,
            }),
        };
        optimise_expr(&mut expr_unary);
        assert_eq!(expr_unary, ExprKind::Var(Ident::of("x")));
    }
}
