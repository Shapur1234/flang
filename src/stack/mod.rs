//! Stack-based intermediate representation

mod instruction;
mod translate;

pub use instruction::Instruction;
/// Translates AST to stack instructions
pub use translate::translate;
