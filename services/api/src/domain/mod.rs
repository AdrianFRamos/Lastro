//! Pure API domain rules.
//!
//! Routes convert HTTP into typed inputs; repositories perform I/O; this module owns
//! semantic rules for capture contexts and evidence-package assembly.

pub mod animals;
pub mod captures;
pub mod evidence;
pub mod v2;
