//! Assembly generation for target architectures

mod amd64;
mod backend;

pub use backend::TargetArch;
pub use backend::generate;
