//! Derivation tree types produced by the parser

use std::{
    borrow::Borrow,
    io::{self, Write},
};

use crate::lexer::token::{Ident, Literal, Op};

/// A node in the derivation tree
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Derivation {
    Ident(Ident),
    Literal(Literal),

    Program(Program),

    Decl(Decl),
    Decls(Vec<Decl>),

    Params(Vec<Param>),
    VarNames(Vec<Ident>),

    Block(Block),
    Statements(Vec<Statement>),
    Statement(Statement),

    Type(Type),

    Expr(Expr),
    ExprTail(Vec<(Op, Expr)>),
    PostfixTail(Vec<PostfixOp>),

    CalledArgs(Vec<Expr>),

    IfTail(Option<Box<Statement>>),
    ForDir(ForDirection),

    ExprTailOption(Option<Expr>),
}

/// Complete derivationt tree
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Program {
    pub name: Ident,
    pub decls: Vec<Decl>,
    pub block: Block,
}

/// Parameter node
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Param {
    pub names: Vec<Ident>,
    pub ty: Type,
}

/// Block node
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Block {
    pub statements: Vec<Statement>,
}

/// Declaration node
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Decl {
    Const {
        names: Vec<Ident>,
        value: Literal,
    },
    Var {
        names: Vec<Ident>,
        ty: Type,
    },
    Proc {
        name: Ident,
        params: Vec<Param>,
        locals: Vec<Decl>,
        block: Block,
    },
    Fn {
        name: Ident,
        params: Vec<Param>,
        return_type: Type,
        locals: Vec<Decl>,
        block: Block,
    },
}

/// Type node
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    Int,
    Real,
    Bool,
    Array {
        from: Literal,
        to: Literal,
        element: Box<Type>,
    },
}

/// Statement node
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Statement {
    Block(Block),
    If {
        cond: Expr,
        then_branch: Box<Statement>,
        else_branch: Option<Box<Statement>>,
    },
    While {
        cond: Expr,
        body: Box<Statement>,
    },
    For {
        name: Ident,
        direction: ForDirection,
        from: Expr,
        to: Expr,
        body: Box<Statement>,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    Expr(Expr),
    Break,
    Continue,
    Exit,
}

/// Direction of a for-loop
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ForDirection {
    To,
    Downto,
}

/// Expression node
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Expr {
    Ident(Ident),
    Literal(Literal),
    Unary { op: Op, expr: Box<Expr> },
    Binary { left: Box<Expr>, op: Op, right: Box<Expr> },
    Index { base: Box<Expr>, index: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
}

/// Postfix operation node applied to an expression
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PostfixOp {
    Index(Expr),
    Call(Vec<Expr>),
}

impl Derivation {
    /// Pretty-prints the derivation tree to `w`.
    pub fn pretty_print(&self, w: &mut impl Write) -> io::Result<()> {
        self.print_recursive(w, "", true)
    }

    fn print_recursive(&self, w: &mut impl Write, prefix: &str, is_last: bool) -> io::Result<()> {
        match self {
            Derivation::Program(p) => p.print_recursive(w, prefix, is_last),
            Derivation::Decl(d) => d.print_recursive(w, prefix, is_last),
            Derivation::Decls(decls) => print_list(w, "Declarations", decls, prefix, is_last),
            Derivation::Block(b) => b.print_recursive(w, prefix, is_last),
            Derivation::Statements(stmts) => print_list(w, "Statements", stmts, prefix, is_last),
            Derivation::Statement(s) => s.print_recursive(w, prefix, is_last),
            Derivation::Expr(e) => e.print_recursive(w, prefix, is_last),
            Derivation::Type(t) => t.print_recursive(w, prefix, is_last),
            Derivation::Ident(i) => print_leaf(w, &format!("Ident: {}", i.0), prefix, is_last),
            Derivation::Literal(l) => print_leaf(w, &format!("Literal: {l:?}"), prefix, is_last),
            Derivation::Params(p) => print_list(w, "Params", p, prefix, is_last),
            Derivation::VarNames(v) => {
                let names: Vec<String> = v.iter().map(|i| i.0.to_string()).collect();
                print_leaf(w, &format!("VarNames: {}", names.join(", ")), prefix, is_last)
            }
            Derivation::IfTail(opt) => {
                if let Some(s) = opt {
                    s.print_recursive(w, prefix, is_last)
                } else {
                    print_leaf(w, "Else: None", prefix, is_last)
                }
            }
            _ => print_leaf(w, &format!("{self:?}"), prefix, is_last),
        }
    }
}

impl Program {
    fn print_recursive(&self, w: &mut impl Write, prefix: &str, is_last: bool) -> io::Result<()> {
        print_node_header(w, &format!("Program: {}", self.name.0), prefix, is_last)?;
        let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

        print_list(w, "Decls", &self.decls, &new_prefix, false)?;
        self.block.print_recursive(w, &new_prefix, true)
    }
}

impl Decl {
    fn print_recursive(&self, w: &mut impl Write, prefix: &str, is_last: bool) -> io::Result<()> {
        match self {
            Decl::Const { names, value } => {
                let ids: Vec<&str> = names.iter().map(|n| n.0.borrow()).collect();
                print_node_header(w, &format!("Const: {}", ids.join(", ")), prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                print_leaf(w, &format!("Val: {value:?}"), &new_prefix, true)
            }
            Decl::Var { names, ty } => {
                let ids: Vec<&str> = names.iter().map(|n| n.0.borrow()).collect();
                print_node_header(w, &format!("Var: {}", ids.join(", ")), prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                ty.print_recursive(w, &new_prefix, true)
            }
            Decl::Proc {
                name,
                params,
                locals,
                block,
            } => {
                print_node_header(w, &format!("Proc: {}", name.0), prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                print_list(w, "Params", params, &new_prefix, false)?;
                print_list(w, "Locals", locals, &new_prefix, false)?;
                block.print_recursive(w, &new_prefix, true)
            }
            Decl::Fn {
                name,
                params,
                return_type,
                locals,
                block,
            } => {
                print_node_header(w, &format!("Fn: {}", name.0), prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                print_list(w, "Params", params, &new_prefix, false)?;
                return_type.print_recursive(w, &new_prefix, false)?;
                print_list(w, "Locals", locals, &new_prefix, false)?;
                block.print_recursive(w, &new_prefix, true)
            }
        }
    }
}

impl Statement {
    fn print_recursive(&self, w: &mut impl Write, prefix: &str, is_last: bool) -> io::Result<()> {
        match self {
            Statement::Block(b) => b.print_recursive(w, prefix, is_last),
            Statement::Assign { target, value } => {
                print_node_header(w, "Assign", prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                target.print_recursive(w, &new_prefix, false)?;
                value.print_recursive(w, &new_prefix, true)
            }
            Statement::Expr(expr) => {
                print_node_header(w, "Expr", prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                expr.print_recursive(w, &new_prefix, true)
            }
            Statement::If {
                cond,
                then_branch,
                else_branch,
            } => {
                print_node_header(w, "If", prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                cond.print_recursive(w, &new_prefix, false)?;
                then_branch.print_recursive(w, &new_prefix, else_branch.is_none())?;
                if let Some(eb) = else_branch {
                    eb.print_recursive(w, &new_prefix, true)?;
                }
                Ok(())
            }
            Statement::While { cond, body } => {
                print_node_header(w, "While", prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                cond.print_recursive(w, &new_prefix, false)?;
                body.print_recursive(w, &new_prefix, true)
            }
            Statement::For {
                name,
                direction,
                from: start,
                to: end,
                body,
            } => {
                print_node_header(w, &format!("For: {} ({:?})", name.0, direction), prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                start.print_recursive(w, &new_prefix, false)?;
                end.print_recursive(w, &new_prefix, false)?;
                body.print_recursive(w, &new_prefix, true)
            }
            Statement::Break => print_leaf(w, "Break", prefix, is_last),
            Statement::Continue => print_leaf(w, "Continue", prefix, is_last),
            Statement::Exit => print_leaf(w, "Exit", prefix, is_last),
        }
    }
}

impl Expr {
    fn print_recursive(&self, w: &mut impl Write, prefix: &str, is_last: bool) -> io::Result<()> {
        match self {
            Expr::Ident(i) => print_leaf(w, &format!("Ident: {}", i.0), prefix, is_last),
            Expr::Literal(l) => print_leaf(w, &format!("Literal: {l:?}"), prefix, is_last),
            Expr::Unary { op, expr } => {
                print_node_header(w, &format!("Unary: {op:?}"), prefix, is_last)?;
                expr.print_recursive(w, &format!("{}{}", prefix, if is_last { "    " } else { "│   " }), true)
            }
            Expr::Binary { left, op, right } => {
                print_node_header(w, &format!("Binary: {op:?}"), prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                left.print_recursive(w, &new_prefix, false)?;
                right.print_recursive(w, &new_prefix, true)
            }
            Expr::Index { base, index } => {
                print_node_header(w, "Index", prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                base.print_recursive(w, &new_prefix, false)?;
                index.print_recursive(w, &new_prefix, true)
            }
            Expr::Call { callee, args } => {
                print_node_header(w, "Call", prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                callee.print_recursive(w, &new_prefix, false)?;
                print_list(w, "Args", args, &new_prefix, true)
            }
        }
    }
}

impl Type {
    fn print_recursive(&self, w: &mut impl Write, prefix: &str, is_last: bool) -> io::Result<()> {
        match self {
            Type::Array {
                from: lower,
                to: upper,
                element,
            } => {
                print_node_header(w, &format!("Array [{lower:?}..{upper:?}]"), prefix, is_last)?;
                element.print_recursive(w, &format!("{}{}", prefix, if is_last { "    " } else { "│   " }), true)
            }
            _ => print_leaf(w, &format!("Type: {self:?}"), prefix, is_last),
        }
    }
}

impl Param {
    fn print_recursive(&self, w: &mut impl Write, prefix: &str, is_last: bool) -> io::Result<()> {
        let names: Vec<&str> = self.names.iter().map(|n| n.0.borrow()).collect();
        print_node_header(w, &format!("Param: {}", names.join(", ")), prefix, is_last)?;
        self.ty
            .print_recursive(w, &format!("{}{}", prefix, if is_last { "    " } else { "│   " }), true)
    }
}

impl Block {
    fn print_recursive(&self, w: &mut impl Write, prefix: &str, is_last: bool) -> io::Result<()> {
        print_list(w, "Block", &self.statements, prefix, is_last)
    }
}

fn print_node_header(w: &mut impl Write, label: &str, prefix: &str, is_last: bool) -> io::Result<()> {
    writeln!(w, "{}{}{}", prefix, if is_last { "└── " } else { "├── " }, label)
}

fn print_leaf(w: &mut impl Write, label: &str, prefix: &str, is_last: bool) -> io::Result<()> {
    writeln!(w, "{}{}{}", prefix, if is_last { "└── " } else { "├── " }, label)
}

fn print_list<T>(w: &mut impl Write, label: &str, items: &[T], prefix: &str, is_last: bool) -> io::Result<()>
where
    T: PrettyPrintNode,
{
    if items.is_empty() {
        return print_leaf(w, &format!("{label}: []"), prefix, is_last);
    }
    print_node_header(w, label, prefix, is_last)?;
    let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
    for (i, item) in items.iter().enumerate() {
        item.print_recursive(w, &new_prefix, i == items.len() - 1)?;
    }
    Ok(())
}

trait PrettyPrintNode {
    fn print_recursive(&self, w: &mut impl Write, prefix: &str, is_last: bool) -> io::Result<()>;
}

impl PrettyPrintNode for Decl {
    fn print_recursive(&self, w: &mut impl Write, p: &str, l: bool) -> io::Result<()> {
        self.print_recursive(w, p, l)
    }
}
impl PrettyPrintNode for Statement {
    fn print_recursive(&self, w: &mut impl Write, p: &str, l: bool) -> io::Result<()> {
        self.print_recursive(w, p, l)
    }
}
impl PrettyPrintNode for Expr {
    fn print_recursive(&self, w: &mut impl Write, p: &str, l: bool) -> io::Result<()> {
        self.print_recursive(w, p, l)
    }
}
impl PrettyPrintNode for Param {
    fn print_recursive(&self, w: &mut impl Write, p: &str, l: bool) -> io::Result<()> {
        self.print_recursive(w, p, l)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    #[test]
    fn test_pretty_print_simple_statement() {
        let statement = Derivation::Statement(Statement::If {
            cond: Expr::Ident(Ident(Cow::Borrowed("x"))),
            then_branch: Box::new(Statement::Break),
            else_branch: None,
        });

        let result = {
            let mut result = Vec::new();
            statement.pretty_print(&mut result).unwrap();
            String::from_utf8(result).unwrap()
        };

        assert_eq!(
            result,
            "\
└── If
    ├── Ident: x
    └── Break\n"
        );
    }
}
