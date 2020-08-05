use std::error::Error;

/// Generic Result.
pub type AVResult<T> = Result<T, Box<dyn Error>>;

pub mod owned;
pub use owned::*;

pub mod writer;
pub use writer::*;
