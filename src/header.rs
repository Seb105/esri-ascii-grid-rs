use std::{
    io::{self, BufRead, BufReader, Read, Seek},
    str::FromStr,
};

use crate::error::Error;

// use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy)]
pub struct EsriASCIIRasterHeader {
    pub ncols: usize,
    pub nrows: usize,
    pub xll: f64,
    pub yll: f64,
    pub yur: f64,
    pub xur: f64,
    pub cornertype: CornerType,
    pub cellsize: f64,
    pub nodata_value: Option<f64>,
}
impl EsriASCIIRasterHeader {
    pub(crate) fn from_reader<R: Seek + Read>(
        reader: &mut BufReader<R>,
    ) -> Result<EsriASCIIRasterHeader, Error> {
        reader.rewind()?;
        let mut lines = reader.lines();

        let ncols = parse_header_line(lines.next(), "ncols")?;
        let nrows = parse_header_line(lines.next(), "nrows")?;

        let (corner_type_x, xll) = parse_ll(lines.next(), "xll")?;
        let (corner_type_y, yll) = parse_ll(lines.next(), "yll")?;
        if corner_type_x != corner_type_y {
            Err(Error::BrokenInvariant("corner type disagree".into()))?
        }

        let cellsize = parse_header_line(lines.next(), "cellsize")?;
        let nodata_value = parse_header_line(lines.next(), "nodata_value").ok();

        Ok(Self {
            ncols,
            nrows,
            xll,
            yll,
            yur: yll + cellsize * (nrows - 1) as f64,
            xur: xll + cellsize * (ncols - 1) as f64,
            cornertype: corner_type_x,
            cellsize,
            nodata_value,
        })
    }
    pub fn num_rows(&self) -> usize {
        self.nrows
    }
    pub fn num_cols(&self) -> usize {
        self.ncols
    }
    pub fn min_x(&self) -> f64 {
        self.xll
    }
    pub fn max_x(&self) -> f64 {
        self.xur
    }
    pub fn min_y(&self) -> f64 {
        self.yll
    }
    pub fn max_y(&self) -> f64 {
        self.yur
    }
    pub fn cell_size(&self) -> f64 {
        self.cellsize
    }
    pub fn no_data_value(&self) -> Option<f64> {
        self.nodata_value
    }
    /// ESRI ASCII files can have either a corner or centre cell type.
    ///
    /// If the cell type is corner, the values are the at coordinates of the bottom left corner of the cell.
    ///
    /// If the cell type is centre, the values are the at coordinates of the centre of the cell.
    pub fn corner_type(&self) -> CornerType {
        self.cornertype
    }
    /// Get the x and y coordinates of the cell at the given row and column, or nothing if it is out of bounds.
    pub fn index_pos(&self, row: usize, col: usize) -> Option<(f64, f64)> {
        let nrows = self.nrows;
        let ncols = self.ncols;
        if row >= nrows || col >= ncols {
            return None;
        }
        let x = self.min_x() + self.cell_size() * (col) as f64;
        let y = self.min_y() + self.cell_size() * (row) as f64;
        Some((x, y))
    }
    /// Get the row and column index of the cell that contains the given x and y, or nothing if it is out of bounds.
    pub fn index_of(&mut self, x: f64, y: f64) -> Option<(usize, usize)> {
        if x < self.min_x() || x > self.max_x() || y < self.min_y() || y > self.max_y() {
            return None;
        }
        let col = ((x - self.min_x()) / self.cellsize).round() as usize;
        let row = ((y - self.min_y()) / self.cellsize).round() as usize;
        Some((col, row))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CornerType {
    Corner,
    Centre,
}
impl FromStr for CornerType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "xllcorner" | "yllcorner" => Ok(Self::Corner),
            "xllcentre" | "yllcentre" => Ok(Self::Centre),
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
    let value = tokens_it
        .next()
        .ok_or_else(|| Error::MissingValue(expected.into()))?
        .parse()?;
    Ok(value)
}

fn parse_ll(
    line: Option<Result<String, io::Error>>,
    expected_prefix: &str,
) -> Result<(CornerType, f64), Error> {
    let expected = format!("{expected_prefix}corner or {expected_prefix}centre");
    let line = line.ok_or_else(|| Error::MissingField(expected.to_owned()))??;
    let mut tokens_it = line.split_whitespace();

    let field = tokens_it
        .next()
        .ok_or_else(|| Error::MissingField(expected.to_owned()))?;
    let corner_type = CornerType::from_str(field)?;

    let value = tokens_it
        .next()
        .ok_or_else(|| Error::MissingValue(expected.to_owned()))?
        .parse()?;
    Ok((corner_type, value))
}
