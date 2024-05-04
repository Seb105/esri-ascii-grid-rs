use crate::error::{self, Error};
use num_traits::{
    Num, NumAssign, NumAssignOps, NumAssignRef,
    NumCast, NumRef,
};
use std::{
    io::{self, BufRead, BufReader, Read, Seek},
    str::FromStr,
    fmt::Debug
};

// use serde::{Serialize, Deserialize};
pub trait Numerical: FromStr<Err = <Self as Numerical>::Err> + Num + NumAssign + NumAssign + NumAssignOps + NumAssignRef + NumRef + NumCast + PartialOrd + PartialEq + Clone + Copy + Debug {
    type Err: Debug;
}
impl<T>Numerical for T 
where T: Num + NumAssign + NumAssign + NumAssignOps + NumAssignRef + NumRef + FromStr + NumCast + PartialOrd + PartialEq + Clone + Copy + Debug,
<T as FromStr>::Err: Debug,  error::Error: From<<T as FromStr>::Err>
{
    type Err = <T as FromStr>::Err;
}

#[derive(Debug, Clone, Copy)]
pub struct EsriASCIIRasterHeader<T: Numerical> {
    pub ncols: usize,
    pub nrows: usize,
    pub xll: T,
    pub yll: T,
    pub yur: T,
    pub xur: T,
    pub cornertype: CornerType,
    pub cellsize: T,
    pub nodata_value: Option<T>,
}
impl<T: Numerical> EsriASCIIRasterHeader<T> where error::Error: From<<T as Numerical>::Err>
{
    pub fn new(
        ncols: usize,
        nrows: usize,
        mut xll: T,
        mut yll: T,
        cornertype: CornerType,
        cellsize: T,
        nodata_value: Option<T>,
    ) -> Self {
        let two: T = T::from(2).unwrap();
        if cornertype == CornerType::Center {
            xll -= cellsize / two;
            yll -= cellsize / two;
        }
        let xur = xll + cellsize * T::from(ncols).unwrap();
        let yur = yll + cellsize * T::from(nrows).unwrap();

        Self {
            ncols,
            nrows,
            xll,
            yll,
            yur,
            xur,
            cornertype,
            cellsize,
            nodata_value,
        }
    }
    pub(crate) fn from_reader<R: Seek + Read>(
        reader: &mut BufReader<R>,
    ) -> Result<EsriASCIIRasterHeader<T>, Error> {
        reader.rewind()?;
        let mut lines = reader.lines();

        let ncols = parse_header_line::<usize>(lines.next(), "ncols")?;
        let nrows = parse_header_line::<usize>(lines.next(), "nrows")?;

        let (corner_type_x, xll) = parse_ll(lines.next(), "xll")?;
        let (corner_type_y, yll) = parse_ll(lines.next(), "yll")?;
        if corner_type_x != corner_type_y {
            Err(Error::BrokenInvariant("corner type disagree".into()))?
        }

        let cellsize = parse_header_line(lines.next(), "cellsize")?;
        let nodata_value = parse_header_line(lines.next(), "nodata_value").ok();

        Ok(Self::new(
            ncols,
            nrows,
            xll,
            yll,
            corner_type_x,
            cellsize,
            nodata_value,
        ))
    }
    pub fn num_rows(&self) -> usize {
        self.nrows
    }
    pub fn num_cols(&self) -> usize {
        self.ncols
    }
    pub fn min_x(&self) -> T {
        self.xll
    }
    pub fn max_x(&self) -> T {
        self.xur
    }
    pub fn min_y(&self) -> T {
        self.yll
    }
    pub fn max_y(&self) -> T {
        self.yur
    }
    pub fn cell_size(&self) -> T {
        self.cellsize
    }
    pub fn no_data_value(&self) -> Option<T> {
        self.nodata_value
    }
    /// ESRI ASCII files can have either a corner or center cell type.
    ///
    /// If the cell type is corner, the values are the at coordinates of the bottom left corner of the cell.
    ///
    /// If the cell type is center, the values are the at coordinates of the center of the cell.
    pub fn corner_type(&self) -> CornerType {
        self.cornertype
    }
    /// Get the x and y coordinates of the cell at the given row and column, or nothing if it is out of bounds.
    pub fn index_pos(&self, row: usize, col: usize) -> Option<(T, T)> {
        let nrows = self.nrows;
        let ncols = self.ncols;
        if row >= nrows || col >= ncols {
            return None;
        }
        let x = self.min_x() + self.cell_size() * T::from(col).unwrap();
        let y = self.min_y() + self.cell_size() * T::from(row).unwrap();
        Some((x, y))
    }
    /// Get the row and column index of the cell that contains the given x and y, or nothing if it is out of bounds.
    pub fn index_of(&self, x: T, y: T) -> Option<(usize, usize)> {
        let max_x = self.max_x();
        let max_y = self.max_y();
        if x < self.min_x() || x > max_x || y < self.min_y() || y > max_y {
            return None;
        }
        let mut col: usize = <usize as NumCast>::from((x - self.min_x()) / self.cellsize).unwrap();
        let mut row: usize = <usize as NumCast>::from((y - self.min_y()) / self.cellsize).unwrap();
        // If the point is on the upper or right edge of the raster, it is considered to be in the last cell.
        if x == max_x {
            col -= 1;
        }
        if y == max_y {
            row -= 1;
        }
        Some((col, row))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CornerType {
    Corner,
    Center,
}
impl FromStr for CornerType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "xllcorner" | "yllcorner" => Ok(Self::Corner),
            "xllcenter" | "yllcenter" => Ok(Self::Center),
            _ => Err(Error::ParseEnum(s.into(), "CornerType")),
        }
    }
}

fn parse_header_line<T>(line: Option<Result<String, io::Error>>, expected: &str) -> Result<T, Error>
where
    T: FromStr,
    Error: From<<T as FromStr>::Err>,
{
    let line = line.ok_or_else(|| Error::MissingField(expected.into()))??;
    let mut tokens_it = line.split_whitespace();

    let field = tokens_it
        .next()
        .ok_or_else(|| Error::MissingField(expected.into()))?;
    if field.to_lowercase() != expected {
        Err(Error::MismatchedField(expected.into(), field.into()))?
    }
    let val_str = tokens_it
        .next()
        .ok_or_else(|| Error::MissingValue(expected.into()))?;
    let value:Result<T, _> = val_str
        .parse()
        .map_err(|_| Error::TypeCast(val_str.into(), field.into(), std::any::type_name::<T>()));
    value
}

fn parse_ll<T>(
    line: Option<Result<String, io::Error>>,
    expected_prefix: &str,
) -> Result<(CornerType, T), Error>
where
    T: FromStr,
    Error: From<<T as FromStr>::Err>,
{
    let expected_prefix = format!("{expected_prefix}corner or {expected_prefix}center");
    let line = line.ok_or_else(|| Error::MissingField(expected_prefix.to_owned()))??;
    let mut tokens_it = line.split_whitespace();

    let field = tokens_it
        .next()
        .ok_or_else(|| Error::MissingField(expected_prefix.to_owned()))?;
    let corner_type = CornerType::from_str(field)?;

    let value_str = tokens_it
        .next()
        .ok_or_else(|| Error::MissingValue(expected_prefix.to_owned()))?;
    let value = value_str
        .parse()
        .map_err(|_| Error::TypeCast(value_str.into(), field.into(), std::any::type_name::<T>()))?;
    Ok((corner_type, value))
}
