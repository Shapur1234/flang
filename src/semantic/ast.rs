use std::{
    borrow::Cow,
    collections::BTreeMap,
    io::{self, Write},
};

use crate::{
    error::SemanticError,
    lexer::token::{Ident, Literal},
    parser::derivation::Type as DType,
};

/// Full validated AST of a compiled program
#[derive(Debug, Clone)]
pub struct Ast {
    pub name: Ident,
    pub vars: BTreeMap<Ident, Type>,
    pub funcs: BTreeMap<Ident, Func>,
    pub entrypoint: Block,
}

impl Ast {
    /// Pretty-prints the AST to `w`
    pub fn pretty_print(&self, w: &mut impl Write) -> io::Result<()> {
        print_node_header(w, &format!("Program {}", self.name.0), "", true)?;
        let prefix = "    ";

        if !self.vars.is_empty() {
            print_node_header(w, "Global Variables", prefix, false)?;
            let inner_prefix = format!("{prefix}│   ");
            for (i, name) in self.vars.keys().enumerate() {
                let ty = &self.vars[name];
                print_leaf(
                    w,
                    &format!("{}: {:?}", name.0, ty),
                    &inner_prefix,
                    i == self.vars.len() - 1,
                )?;
            }
        }

        for (fname, func) in &self.funcs {
            func.print_recursive(w, fname, prefix, false)?;
        }

        print_node_header(w, "Entrypoint", prefix, true)?;
        self.entrypoint.print_recursive(w, &format!("{prefix}    "), true)
    }
}

/// Validated Type
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    Void,
    Int,
    Real,
    Bool,
    Array { from: i64, to: i64, element: Box<Type> },
}

impl Type {
    /// Computes the size of the type in bytes
    pub const fn size(&self) -> u64 {
        match self {
            Type::Void => 0,
            Type::Int | Type::Real | Type::Bool => 8,
            Type::Array { from, to, element } => (to.saturating_sub(*from).unsigned_abs() + 1) * element.size(),
        }
    }
}

impl TryFrom<DType> for Type {
    type Error = SemanticError;

    /// Converts a parser [`DType`] to AST [`Type`], validating array bounds
    fn try_from(value: DType) -> Result<Type, Self::Error> {
        match value {
            DType::Int => Ok(Type::Int),
            DType::Real => Ok(Type::Real),
            DType::Bool => Ok(Type::Bool),
            DType::Array { from, to, element } => {
                let element_type = Type::try_from(*element)?;
                match (&from, &to) {
                    (Literal::Int(from_int), Literal::Int(to_int)) if from_int < to_int => Ok(Type::Array {
                        from: *from_int,
                        to: *to_int,
                        element: Box::new(element_type),
                    }),
                    _ => Err(SemanticError::IllegalArrayBounds { from, to }),
                }
            }
        }
    }
}

/// Parameter in function/procedure declaration
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Param {
    pub name: Ident,
    pub ty: Type,
}

/// Function or procedure definition
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Func {
    pub params: Cow<'static, [Param]>,
    pub return_ty: Type,
    pub vars: BTreeMap<Ident, Type>,
    pub block: Block,
}

impl Func {
    /// Pretty-prints the function to `w`
    fn print_recursive(&self, w: &mut impl Write, name: &Ident, prefix: &str, is_last: bool) -> io::Result<()> {
        print_node_header(w, &format!("Func: {} -> {:?}", name.0, self.return_ty), prefix, is_last)?;
        let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

        if !self.params.is_empty() {
            print_node_header(w, "Parameters", &new_prefix, false)?;
            let p_prefix = format!("{new_prefix}│   ");
            for (i, param) in self.params.iter().enumerate() {
                print_leaf(
                    w,
                    &format!("{}: {:?}", param.name.0, param.ty),
                    &p_prefix,
                    i == self.params.len() - 1,
                )?;
            }
        }

        if !self.vars.is_empty() {
            print_node_header(w, "Local Variables", &new_prefix, false)?;
            let v_prefix = format!("{new_prefix}│   ");
            for (i, name) in self.vars.keys().enumerate() {
                let ty = &self.vars[name];
                print_leaf(w, &format!("{}: {:?}", name.0, ty), &v_prefix, i == self.vars.len() - 1)?;
            }
        }

        self.block.print_recursive(w, &new_prefix, true)
    }
}

/// Block node
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Block {
    pub statements: Vec<Statement>,
}

impl Block {
    fn print_recursive(&self, w: &mut impl Write, prefix: &str, is_last: bool) -> io::Result<()> {
        print_node_header(w, "Block", prefix, is_last)?;
        let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
        for (i, stmt) in self.statements.iter().enumerate() {
            stmt.print_recursive(w, &new_prefix, i == self.statements.len() - 1)?;
        }
        Ok(())
    }
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

/// Direction of a for-loop
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ForDirection {
    To,
    Downto,
}

/// Expression with type information
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Type,
    pub is_lvalue: bool,
}

/// Kind (body) of an expression
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExprKind {
    Var(Ident),
    Literal(Literal),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Return,
}

impl Expr {
    fn print_recursive(&self, w: &mut impl Write, prefix: &str, is_last: bool) -> io::Result<()> {
        match &self.kind {
            ExprKind::Var(i) => print_leaf(w, &format!("Ident: {}", i.0), prefix, is_last),
            ExprKind::Literal(l) => print_leaf(w, &format!("Literal: {l:?}"), prefix, is_last),
            ExprKind::Unary { op, expr } => {
                print_node_header(w, &format!("Unary: {op:?}"), prefix, is_last)?;
                expr.print_recursive(w, &format!("{}{}", prefix, if is_last { "    " } else { "│   " }), true)
            }
            ExprKind::Binary { left, op, right } => {
                print_node_header(w, &format!("Binary: {op:?}"), prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                left.print_recursive(w, &new_prefix, false)?;
                right.print_recursive(w, &new_prefix, true)
            }
            ExprKind::Index { base, index } => {
                print_node_header(w, "Index", prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                base.print_recursive(w, &new_prefix, false)?;
                index.print_recursive(w, &new_prefix, true)
            }
            ExprKind::Call { callee, args } => {
                print_node_header(w, "Call", prefix, is_last)?;
                let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                callee.print_recursive(w, &new_prefix, args.is_empty())?;
                for (i, arg) in args.iter().enumerate() {
                    arg.print_recursive(w, &new_prefix, i == args.len() - 1)?;
                }
                Ok(())
            }
            ExprKind::Return => print_leaf(w, "Return", prefix, is_last),
        }
    }
}

/// Unary operator
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnaryOp {
    Minus,
    Not,
    Plus,
}

/// Binary operator
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BinaryOp {
    And,
    Div,
    Eq,
    Equiv,
    Geq,
    Gt,
    Imply,
    Leq,
    Lt,
    Minus,
    Mod,
    Neq,
    Or,
    Plus,
    Times,
    Xor,
}

fn print_node_header(w: &mut impl Write, label: &str, prefix: &str, is_last: bool) -> io::Result<()> {
    writeln!(w, "{}{}{}", prefix, if is_last { "└── " } else { "├── " }, label)
}

fn print_leaf(w: &mut impl Write, label: &str, prefix: &str, is_last: bool) -> io::Result<()> {
    writeln!(w, "{}{}{}", prefix, if is_last { "└── " } else { "├── " }, label)
}
