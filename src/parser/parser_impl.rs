use crate::{
    error::{CompilationError, ParserError},
    lexer::token::{Op, PositionedToken, Token},
    parser::{
        action::Action,
        derivation::{Block, Decl, Derivation, Expr, ForDirection, Param, PostfixOp, Program, Statement, Type},
        non_terminal::NonTerminal,
        stack_symbol::StackSymbol,
        tables::{PARSING_TABLE, RULES},
    },
};

macro_rules! pop_sem {
    ($parser:expr, $variant:path) => {
        match $parser.semantic_stack.pop().expect("Semantic stack underflow") {
            $variant(v) => v,
            other => {
                panic!(
                    "Semantic stack mismatch: expected {}, found {:?}",
                    stringify!($variant),
                    other
                );
            }
        }
    };
}

/// Parses tokens using LL(1) parsing table
///
/// Returns a derivation tree or error
pub fn parse(
    tokens: impl Iterator<Item = Result<PositionedToken, CompilationError>>,
) -> Result<Program, CompilationError> {
    let mut parser = Parser::new(tokens)?;
    parser.parse()
}

struct Parser<I: Iterator<Item = Result<PositionedToken, CompilationError>>> {
    input: I,
    current: Option<PositionedToken>,
    parsing_stack: Vec<StackSymbol>,
    semantic_stack: Vec<Derivation>,
}

impl<I: Iterator<Item = Result<PositionedToken, CompilationError>>> Parser<I> {
    fn new(mut input: I) -> Result<Self, CompilationError> {
        let current = input.next().transpose()?;
        Ok(Self {
            input,
            current,
            parsing_stack: vec![StackSymbol::NonTerminal(NonTerminal::S)],
            semantic_stack: Vec::new(),
        })
    }

    fn parse(&mut self) -> Result<Program, CompilationError> {
        while let Some(top) = self.parsing_stack.pop() {
            match top {
                StackSymbol::Terminal(expected) => self.consume_terminal(expected)?,
                StackSymbol::NonTerminal(nt) => self.expand_non_terminal(&nt)?,
                StackSymbol::Action(action) => self.execute_action(&action),
            }
        }

        if let Some(token) = self.current.as_ref() {
            return Err(CompilationError::Parser(ParserError::ExpectedEnd {
                found: token.clone(),
            }));
        }

        if self.semantic_stack.len() > 1 {
            return Err(CompilationError::Parser(ParserError::UnexpectedEnd));
        }

        Ok(pop_sem!(self, Derivation::Program))
    }

    fn consume_terminal(&mut self, expected: Token) -> Result<(), CompilationError> {
        let current = self
            .current
            .as_ref()
            .ok_or(CompilationError::Parser(ParserError::UnexpectedEnd))?;

        if !expected.kind_equals(&current.payload) {
            return Err(CompilationError::Parser(ParserError::ExpectedToken {
                expected,
                found: current.clone(),
            }));
        }

        match &current.payload {
            Token::Ident(id) => self.semantic_stack.push(Derivation::Ident(id.clone())),
            Token::Literal(lit) => self.semantic_stack.push(Derivation::Literal(lit.clone())),
            _ => {}
        }

        self.current = self.input.next().transpose()?;
        Ok(())
    }

    fn expand_non_terminal(&mut self, non_terminal: &NonTerminal) -> Result<(), CompilationError> {
        let current = self
            .current
            .as_ref()
            .ok_or(CompilationError::Parser(ParserError::UnexpectedEnd))?;

        let rule_id = PARSING_TABLE[&non_terminal].get(&current.payload).ok_or_else(|| {
            if let Some(token) = &self.current {
                CompilationError::Parser(ParserError::UnexpectedToken { token: token.clone() })
            } else {
                CompilationError::Parser(ParserError::UnexpectedEnd)
            }
        })?;

        for sym in RULES[rule_id].iter().rev() {
            self.parsing_stack.push(sym.clone());
        }

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn execute_action(&mut self, action: &Action) {
        match action {
            Action::Program => {
                let block = pop_sem!(self, Derivation::Block);
                let mut decls = pop_sem!(self, Derivation::Decls);
                decls.reverse();

                let name = pop_sem!(self, Derivation::Ident);
                self.semantic_stack
                    .push(Derivation::Program(Program { name, decls, block }));
            }
            Action::JoinDecls => {
                let mut tail = pop_sem!(self, Derivation::Decls);
                let decls = pop_sem!(self, Derivation::Decls);
                tail.extend(decls);

                self.semantic_stack.push(Derivation::Decls(tail));
            }
            Action::InsertDecl => {
                let mut tail = pop_sem!(self, Derivation::Decls);
                let decl = pop_sem!(self, Derivation::Decl);

                tail.push(decl);
                self.semantic_stack.push(Derivation::Decls(tail));
            }
            Action::InsertConstDecl => {
                let mut tail = pop_sem!(self, Derivation::Decls);
                let value = pop_sem!(self, Derivation::Literal);

                let mut names = pop_sem!(self, Derivation::VarNames);
                names.reverse();

                tail.push(Decl::Const { names, value });
                self.semantic_stack.push(Derivation::Decls(tail));
            }
            Action::InsertVarDecl => {
                let mut tail = pop_sem!(self, Derivation::Decls);
                let ty = pop_sem!(self, Derivation::Type);

                let mut names = pop_sem!(self, Derivation::VarNames);
                names.reverse();

                tail.push(Decl::Var { names, ty });
                self.semantic_stack.push(Derivation::Decls(tail));
            }
            Action::EmptyDecls => {
                self.semantic_stack.push(Derivation::Decls(Vec::new()));
            }
            Action::ProcDecl => {
                let block = pop_sem!(self, Derivation::Block);

                let mut locals = pop_sem!(self, Derivation::Decls);
                locals.reverse();

                let mut params = pop_sem!(self, Derivation::Params);
                params.reverse();

                let name = pop_sem!(self, Derivation::Ident);

                self.semantic_stack.push(Derivation::Decl(Decl::Proc {
                    name,
                    params,
                    locals,
                    block,
                }));
            }
            Action::FnDecl => {
                let block = pop_sem!(self, Derivation::Block);

                let mut locals = pop_sem!(self, Derivation::Decls);
                locals.reverse();

                let return_type = pop_sem!(self, Derivation::Type);

                let mut params = pop_sem!(self, Derivation::Params);
                params.reverse();

                let name = pop_sem!(self, Derivation::Ident);

                self.semantic_stack.push(Derivation::Decl(Decl::Fn {
                    name,
                    params,
                    return_type,
                    locals,
                    block,
                }));
            }
            Action::TypeInt => {
                self.semantic_stack.push(Derivation::Type(Type::Int));
            }
            Action::TypeReal => {
                self.semantic_stack.push(Derivation::Type(Type::Real));
            }
            Action::TypeBool => {
                self.semantic_stack.push(Derivation::Type(Type::Bool));
            }
            Action::TypeArray => {
                let element = pop_sem!(self, Derivation::Type);
                let upper = pop_sem!(self, Derivation::Literal);
                let lower = pop_sem!(self, Derivation::Literal);

                self.semantic_stack.push(Derivation::Type(Type::Array {
                    from: lower,
                    to: upper,
                    element: Box::new(element),
                }));
            }
            Action::InsertArg => {
                let mut tail = pop_sem!(self, Derivation::Params);

                let ty = pop_sem!(self, Derivation::Type);
                let name = pop_sem!(self, Derivation::Ident);
                tail.push(Param { names: vec![name], ty });

                self.semantic_stack.push(Derivation::Params(tail));
            }
            Action::EmptyParams => {
                self.semantic_stack.push(Derivation::Params(Vec::new()));
            }
            Action::InsertIdent => {
                let mut tail = pop_sem!(self, Derivation::VarNames);

                let name = pop_sem!(self, Derivation::Ident);
                tail.push(name);

                self.semantic_stack.push(Derivation::VarNames(tail));
            }
            Action::EmptyVarNames => {
                self.semantic_stack.push(Derivation::VarNames(Vec::new()));
            }

            Action::Block => {
                let mut statements = pop_sem!(self, Derivation::Statements);
                statements.reverse();

                self.semantic_stack.push(Derivation::Block(Block { statements }));
            }
            Action::InsertStatement => {
                let mut tail = pop_sem!(self, Derivation::Statements);
                let statement = pop_sem!(self, Derivation::Statement);

                tail.push(statement);

                self.semantic_stack.push(Derivation::Statements(tail));
            }
            Action::EmptyStatements => {
                self.semantic_stack.push(Derivation::Statements(Vec::new()));
            }

            Action::StatementBlock => {
                let block = pop_sem!(self, Derivation::Block);
                self.semantic_stack.push(Derivation::Statement(Statement::Block(block)));
            }

            Action::StatementBreak => {
                self.semantic_stack.push(Derivation::Statement(Statement::Break));
            }
            Action::StatementContinue => {
                self.semantic_stack.push(Derivation::Statement(Statement::Continue));
            }
            Action::StatementExit => {
                self.semantic_stack.push(Derivation::Statement(Statement::Exit));
            }
            Action::ExprTail => {
                let value = pop_sem!(self, Derivation::Expr);
                self.semantic_stack.push(Derivation::ExprTailOption(Some(value)));
            }
            Action::ExprTailEmpty => {
                self.semantic_stack.push(Derivation::ExprTailOption(None));
            }
            Action::StatementExpr => {
                let tail = pop_sem!(self, Derivation::ExprTailOption);
                let expr = pop_sem!(self, Derivation::Expr);

                let statement = match tail {
                    Some(value) => Statement::Assign { target: expr, value },
                    None => Statement::Expr(expr),
                };
                self.semantic_stack.push(Derivation::Statement(statement));
            }
            Action::StatementIf => {
                let else_branch = pop_sem!(self, Derivation::IfTail);
                let then_branch = pop_sem!(self, Derivation::Statement);
                let cond = pop_sem!(self, Derivation::Expr);

                self.semantic_stack.push(Derivation::Statement(Statement::If {
                    cond,
                    then_branch: Box::new(then_branch),
                    else_branch,
                }));
            }
            Action::IfTail => {
                let statement = pop_sem!(self, Derivation::Statement);

                self.semantic_stack.push(Derivation::IfTail(Some(Box::new(statement))));
            }
            Action::EmptyIfTail => {
                self.semantic_stack.push(Derivation::IfTail(None));
            }
            Action::StatementWhile => {
                let body = pop_sem!(self, Derivation::Statement);
                let cond = pop_sem!(self, Derivation::Expr);

                self.semantic_stack.push(Derivation::Statement(Statement::While {
                    cond,
                    body: Box::new(body),
                }));
            }
            Action::StatementFor => {
                let body = pop_sem!(self, Derivation::Statement);
                let end = pop_sem!(self, Derivation::Expr);
                let direction = pop_sem!(self, Derivation::ForDir);
                let start = pop_sem!(self, Derivation::Expr);
                let name = pop_sem!(self, Derivation::Ident);

                self.semantic_stack.push(Derivation::Statement(Statement::For {
                    name,
                    direction,
                    from: start,
                    to: end,
                    body: Box::new(body),
                }));
            }
            Action::ForDirTo => {
                self.semantic_stack.push(Derivation::ForDir(ForDirection::To));
            }
            Action::ForDirDownto => {
                self.semantic_stack.push(Derivation::ForDir(ForDirection::Downto));
            }
            Action::InsertCalledArgs => {
                let mut tail = pop_sem!(self, Derivation::CalledArgs);
                let expr = pop_sem!(self, Derivation::Expr);

                tail.push(expr);

                self.semantic_stack.push(Derivation::CalledArgs(tail));
            }
            Action::EmptyCalledArgs => {
                self.semantic_stack.push(Derivation::CalledArgs(Vec::new()));
            }
            Action::Expr => {
                let mut tail = pop_sem!(self, Derivation::ExprTail);
                tail.reverse();

                let lhs = pop_sem!(self, Derivation::Expr);

                self.semantic_stack.push(Derivation::Expr({
                    let mut expr = lhs;
                    for (op, rhs) in tail {
                        expr = Expr::Binary {
                            left: Box::new(expr),
                            op,
                            right: Box::new(rhs),
                        };
                    }
                    expr
                }));
            }
            Action::TailEquiv => self.prepend_binary_tail(Op::Equiv),
            Action::TailImply => self.prepend_binary_tail(Op::Imply),
            Action::TailOr => self.prepend_binary_tail(Op::Or),
            Action::TailXor => self.prepend_binary_tail(Op::Xor),
            Action::TailAnd => self.prepend_binary_tail(Op::And),
            Action::TailPlus => self.prepend_binary_tail(Op::Plus),
            Action::TailMinus => self.prepend_binary_tail(Op::Minus),
            Action::TailTimes => self.prepend_binary_tail(Op::Times),
            Action::TailDiv => self.prepend_binary_tail(Op::Div),
            Action::TailMod => self.prepend_binary_tail(Op::Mod),
            Action::TailEq => self.binary_tail_of(Op::Eq),
            Action::TailNeq => self.binary_tail_of(Op::Neq),
            Action::TailLt => self.binary_tail_of(Op::Lt),
            Action::TailGt => self.binary_tail_of(Op::Gt),
            Action::TailLeq => self.binary_tail_of(Op::Leq),
            Action::TailGeq => self.binary_tail_of(Op::Geq),
            Action::TailEmpty => {
                self.semantic_stack.push(Derivation::ExprTail(Vec::new()));
            }
            Action::UnaryPlus | Action::UnaryMinus | Action::UnaryNot => {
                let expr = pop_sem!(self, Derivation::Expr);
                let op = match action {
                    Action::UnaryPlus => Op::Plus,
                    Action::UnaryMinus => Op::Minus,
                    Action::UnaryNot => Op::Not,
                    _ => unreachable!(),
                };

                self.semantic_stack.push(Derivation::Expr(Expr::Unary {
                    op,
                    expr: Box::new(expr),
                }));
            }

            Action::AtomIdent => {
                let tail = pop_sem!(self, Derivation::PostfixTail);
                let ident = pop_sem!(self, Derivation::Ident);

                self.semantic_stack
                    .push(Derivation::Expr(Self::fold_postfix_tail(Expr::Ident(ident), tail)));
            }
            Action::AtomLiteral => {
                let tail = pop_sem!(self, Derivation::PostfixTail);
                let literal = pop_sem!(self, Derivation::Literal);

                self.semantic_stack
                    .push(Derivation::Expr(Self::fold_postfix_tail(Expr::Literal(literal), tail)));
            }
            Action::AtomGroup => {
                let tail = pop_sem!(self, Derivation::PostfixTail);
                let expr = pop_sem!(self, Derivation::Expr);

                self.semantic_stack
                    .push(Derivation::Expr(Self::fold_postfix_tail(expr, tail)));
            }
            Action::InsertIndex => {
                let mut tail = pop_sem!(self, Derivation::PostfixTail);

                let index = pop_sem!(self, Derivation::Expr);
                tail.push(PostfixOp::Index(index));

                self.semantic_stack.push(Derivation::PostfixTail(tail));
            }
            Action::InsertCall => {
                let mut tail = pop_sem!(self, Derivation::PostfixTail);

                let mut args = pop_sem!(self, Derivation::CalledArgs);
                args.reverse();

                tail.push(PostfixOp::Call(args));

                self.semantic_stack.push(Derivation::PostfixTail(tail));
            }
            Action::EmptyPostfixTail => {
                self.semantic_stack.push(Derivation::PostfixTail(Vec::new()));
            }
        }
    }

    fn prepend_binary_tail(&mut self, op: Op) {
        let mut tail = pop_sem!(self, Derivation::ExprTail);
        let rhs = pop_sem!(self, Derivation::Expr);
        tail.push((op, rhs));

        self.semantic_stack.push(Derivation::ExprTail(tail));
    }

    fn binary_tail_of(&mut self, op: Op) {
        let rhs = pop_sem!(self, Derivation::Expr);

        self.semantic_stack.push(Derivation::ExprTail(vec![(op, rhs)]));
    }

    fn fold_postfix_tail(base: Expr, mut tail: Vec<PostfixOp>) -> Expr {
        tail.reverse();

        let mut expr = base;
        for op in tail {
            expr = match op {
                PostfixOp::Index(index) => Expr::Index {
                    base: Box::new(expr),
                    index: Box::new(index),
                },
                PostfixOp::Call(args) => Expr::Call {
                    callee: Box::new(expr),
                    args,
                },
            };
        }
        expr
    }
}

#[cfg(test)]
mod parser_tests {
    use std::borrow::Cow;

    use super::*;
    use crate::lexer::token::{Ident, Keyword, Literal, Punct};

    #[allow(clippy::unnecessary_wraps)]
    fn token(payload: Token) -> Result<PositionedToken, CompilationError> {
        Ok(PositionedToken {
            payload,
            line: 1,
            column: 1,
        })
    }

    #[test]
    fn test_valid_empty_program() {
        let tokens = [
            token(Token::Keyword(Keyword::Program)),
            token(Token::Ident(Ident(Cow::Borrowed("TEST")))),
            token(Token::Punct(Punct::Semicolon)),
            token(Token::Punct(Punct::LCurlyBracket)),
            token(Token::Punct(Punct::RCurlyBracket)),
            token(Token::Punct(Punct::Dot)),
        ];

        let result = parse(tokens.into_iter());
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.name.0, "TEST");
        assert!(result.decls.is_empty());
        assert!(result.block.statements.is_empty());
    }

    #[test]
    fn test_invalid_syntax_yields_parser_error() {
        let tokens = [
            token(Token::Keyword(Keyword::Program)),
            token(Token::Ident(Ident(Cow::Borrowed("TEST")))),
            token(Token::Punct(Punct::Semicolon)),
            token(Token::Punct(Punct::Dot)),
        ];

        let result = parse(tokens.into_iter());

        match result {
            Err(CompilationError::Parser(ParserError::UnexpectedToken { .. })) => (),
            other => panic!("Expected UnexpectedToken error, got {other:?}"),
        }
    }

    #[test]
    fn test_unexpected_eof() {
        let tokens = [
            token(Token::Keyword(Keyword::Program)),
            token(Token::Ident(Ident(Cow::Borrowed("TEST")))),
            token(Token::Punct(Punct::Semicolon)),
            token(Token::Punct(Punct::LCurlyBracket)),
        ];

        let result = parse(tokens.into_iter());
        match result {
            Err(CompilationError::Parser(ParserError::UnexpectedEnd)) => (),
            other => panic!("Expected UnexpectedEnd error, got {other:?}"),
        }
    }

    #[test]
    fn test_left_associativity_expr() {
        // Test parsing of `A - B - C` ensures that left-associativity via push/reverse is accurate
        let tokens = [
            token(Token::Keyword(Keyword::Program)),
            token(Token::Ident(Ident(Cow::Borrowed("TEST")))),
            token(Token::Punct(Punct::Semicolon)),
            token(Token::Punct(Punct::LCurlyBracket)),
            // A - B - C;
            token(Token::Ident(Ident(Cow::Borrowed("A")))),
            token(Token::Op(Op::Minus)),
            token(Token::Ident(Ident(Cow::Borrowed("B")))),
            token(Token::Op(Op::Minus)),
            token(Token::Ident(Ident(Cow::Borrowed("C")))),
            token(Token::Punct(Punct::Semicolon)),
            token(Token::Punct(Punct::RCurlyBracket)),
            token(Token::Punct(Punct::Dot)),
        ];

        let result = parse(tokens.into_iter()).unwrap();

        let stmt = &result.block.statements[0];
        if let Statement::Expr(Expr::Binary { left, right, .. }) = stmt {
            assert!(matches!(**right, Expr::Ident(Ident(Cow::Borrowed("C")))));

            if let Expr::Binary {
                left: inner_left,
                right: inner_right,
                ..
            } = &**left
            {
                assert!(matches!(**inner_left, Expr::Ident(Ident(Cow::Borrowed("A")))));
                assert!(matches!(**inner_right, Expr::Ident(Ident(Cow::Borrowed("B")))));
            } else {
                panic!("Inner node should be Binary expression");
            }
        } else {
            panic!("Expected Binary expression");
        }
    }

    #[test]
    fn test_declaration_ordering() {
        let tokens = [
            token(Token::Keyword(Keyword::Program)),
            token(Token::Ident(Ident(Cow::Borrowed("TEST")))),
            token(Token::Punct(Punct::Semicolon)),
            // const C = 1;
            token(Token::Keyword(Keyword::Const)),
            token(Token::Ident(Ident(Cow::Borrowed("C")))),
            token(Token::Op(Op::Eq)),
            token(Token::Literal(Literal::Int(1))),
            token(Token::Punct(Punct::Semicolon)),
            // var V: Int;
            token(Token::Keyword(Keyword::Var)),
            token(Token::Ident(Ident(Cow::Borrowed("V")))),
            token(Token::Punct(Punct::Colon)),
            token(Token::Keyword(Keyword::Int)),
            token(Token::Punct(Punct::Semicolon)),
            // block
            token(Token::Punct(Punct::LCurlyBracket)),
            token(Token::Punct(Punct::RCurlyBracket)),
            token(Token::Punct(Punct::Dot)),
        ];

        let result = parse(tokens.into_iter()).unwrap();
        assert_eq!(result.decls.len(), 2);

        // Decls should be in textual order: Const then Var
        assert!(matches!(result.decls[0], Decl::Const { .. }));
        assert!(matches!(result.decls[1], Decl::Var { .. }));
    }

    #[test]
    fn test_deep_chained_operations() {
        // x[1][2](3, 4) := 5;
        let tokens = [
            token(Token::Keyword(Keyword::Program)),
            token(Token::Ident(Ident(Cow::Borrowed("TEST")))),
            token(Token::Punct(Punct::Semicolon)),
            token(Token::Punct(Punct::LCurlyBracket)),
            // x[1][2](3, 4)
            token(Token::Ident(Ident(Cow::Borrowed("x")))),
            token(Token::Punct(Punct::LSquareBracket)),
            token(Token::Literal(Literal::Int(1))),
            token(Token::Punct(Punct::RSquareBracket)),
            token(Token::Punct(Punct::LSquareBracket)),
            token(Token::Literal(Literal::Int(2))),
            token(Token::Punct(Punct::RSquareBracket)),
            token(Token::Punct(Punct::LBracket)),
            token(Token::Literal(Literal::Int(3))),
            token(Token::Punct(Punct::Comma)),
            token(Token::Literal(Literal::Int(4))),
            token(Token::Punct(Punct::RBracket)),
            // := 5;
            token(Token::Op(Op::Assign)),
            token(Token::Literal(Literal::Int(5))),
            token(Token::Punct(Punct::Semicolon)),
            token(Token::Punct(Punct::RCurlyBracket)),
            token(Token::Punct(Punct::Dot)),
        ];

        let result = parse(tokens.into_iter()).unwrap();
        let stmt = &result.block.statements[0];

        if let Statement::Assign { target, value } = stmt {
            assert!(matches!(value, Expr::Literal(Literal::Int(5))));

            if let Expr::Call { callee, args } = target {
                assert_eq!(args.len(), 2); // 3, 4

                if let Expr::Index {
                    base: inner_callee,
                    index: outer_idx,
                } = &**callee
                {
                    assert!(matches!(**outer_idx, Expr::Literal(Literal::Int(2))));

                    if let Expr::Index {
                        base: core_callee,
                        index: inner_idx,
                    } = &**inner_callee
                    {
                        assert!(matches!(**inner_idx, Expr::Literal(Literal::Int(1))));
                        assert!(matches!(**core_callee, Expr::Ident(Ident(Cow::Borrowed("x")))));
                    } else {
                        panic!("Expected second index node");
                    }
                } else {
                    panic!("Expected first index node");
                }
            } else {
                panic!("Expected call node");
            }
        } else {
            panic!("Expected assignment statement");
        }
    }
}
