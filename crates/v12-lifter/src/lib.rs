//! v12-lifter: lowers the Oxc AST into V12 SSA IR.

pub mod lifter;
pub mod scope;

pub use lifter::Lifter;

#[cfg(test)]
mod ir_dump_test;
