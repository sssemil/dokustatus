pub mod adapters;
pub mod application;
pub mod domain;
pub mod infra;

// Test utilities (available in all builds for integration tests)
#[cfg(test)]
pub mod test_utils;

// Re-exports for shorter use statements.
pub use application::*;
pub use domain::*;
