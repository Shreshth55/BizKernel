//! BizKernel core: the guardrail. The agent proposes `Operation`s; the
//! `Kernel` is the sole writer and refuses anything that breaks integrity.

pub mod domain;
pub mod kernel;
pub mod ops;

pub use domain::*;
pub use kernel::{Kernel, ReplayError};
pub use ops::{Event, Operation, Rejection};
