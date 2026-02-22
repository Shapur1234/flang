//! Semantic analysis: type checking, AST construction, optimization

pub mod ast;
mod builtin;
mod check;
mod optimisation;

pub use builtin::BUILTIN_FUNCS;
pub use check::check;
pub use optimisation::optimise;
