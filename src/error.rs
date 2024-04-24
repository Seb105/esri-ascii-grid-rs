use std::{
    io,
    num::{ParseFloatError, ParseIntError},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Field {0} is expected but missing")]
    MissingField(String),

    #[error("Expecting {0}; got {1}")]
    MismatchedField(String, String),

    #[error("Field {0} expects a value but it's missing")]
    MissingValue(String),

    #[error("An invariant is violated: {0}")]
    BrokenInvariant(String),

    #[error("{0} is not a valid enum variant for {1}")]
    ParseEnum(String, &'static str),

    #[error("Expecting an integer: {0}")]
    ParseInt(#[from] ParseIntError),

    #[error("Expecting a float: {0}")]
    ParseFloat(#[from] ParseFloatError),

    #[error("Expecting {0} rows; got {1}")]
    MismatchedRowCount(usize, usize),

    #[error("The given index ({0}, {1}) is out of bounds")]
    OutOfBounds(usize, usize),
}
