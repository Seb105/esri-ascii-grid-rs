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

    #[cfg(feature = "ordered-float")]
    #[error("A parsed float value resulted in a NaN")]
    FloatIsNan,

    #[error("Expecting {0} rows; got {1}")]
    MismatchedRowCount(usize, usize),

    #[error("Expecting {0} columns; got {1}")]
    MismatchColumnCount(usize, usize),

    #[error("The given index ({0}, {1}) is out of bounds")]
    OutOfBounds(usize, usize),

    #[error("The value {0} in {1} cannot be represented as type {2}")]
    TypeCast(String, String, &'static str),
}

#[cfg(feature = "ordered-float")]
impl From<ordered_float::ParseNotNanError<ParseFloatError>> for Error {
    fn from(err: ordered_float::ParseNotNanError<ParseFloatError>) -> Self {
        use ordered_float::ParseNotNanError as E;
        match err {
            E::ParseFloatError(e) => Self::ParseFloat(e),
            E::IsNaN => Self::FloatIsNan,
        }
    }
}
