use std::borrow::Cow;

use crate::{asm::amd64::Amd64Generator, stack::Instruction};

/// Target architecture for code generation
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TargetArch {
    Amd64,
}

/// Alias for an iterator of Instructions
pub trait InstructionsIterator = Iterator<Item = Instruction>;
/// Alias for an iterator of output lines
pub trait OutputLinesIterator = Iterator<Item = Cow<'static, str>>;

/// Generates target-specific assembly from stack instructions
/// Returns an iterator of assembly lines.
pub fn generate<S: Iterator<Item = Instruction>>(
    target_arch: &TargetArch,
    instructions: S,
) -> impl OutputLinesIterator {
    match target_arch {
        TargetArch::Amd64 => Amd64Generator::new(instructions),
    }
}
