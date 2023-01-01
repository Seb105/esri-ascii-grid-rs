use std::{
    io::{BufRead, BufReader, Error, Read, Seek},
    str::FromStr,
};

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
        let ncols = parse_header_line::<usize>(lines.next(), "ncols")
            .ok_or_else(|| Error::new(std::io::ErrorKind::InvalidData, "ncols invalid"))?;
        let nrows = parse_header_line::<usize>(lines.next(), "nrows")
            .ok_or_else(|| Error::new(std::io::ErrorKind::InvalidData, "nrows invalid"))?;
        let (corner_type, xll) = parse_ll(lines.next())
            .ok_or_else(|| Error::new(std::io::ErrorKind::InvalidData, "xll invalid"))?;
        let (_, yll) = parse_ll(lines.next())
            .ok_or_else(|| Error::new(std::io::ErrorKind::InvalidData, "yll invalid"))?;
        let cellsize = parse_header_line::<f64>(lines.next(), "cellsize")
            .ok_or_else(|| Error::new(std::io::ErrorKind::InvalidData, "cellsize invalid"))?;
        let nodata_value = parse_header_line::<f64>(lines.next(), "nodata_value");
        Ok(Self {
            ncols,
            nrows,
            xll,
            yll,
            yur: yll + cellsize * (nrows - 1) as f64,
            xur: xll + cellsize * (ncols - 1) as f64,
            cornertype: corner_type,
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
impl CornerType {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "xllcorner" | "yllcorner" => Some(Self::Corner),
            "xllcentre" | "yllcentre" => Some(Self::Centre),
            _ => None,
        }
    }
}
fn parse_header_line<T: FromStr>(line: Option<Result<String, Error>>, expected: &str) -> Option<T> {
    let line = line.and_then(Result::ok)?;
    let split = line.split_whitespace().collect::<Vec<&str>>();
    if expected != split.first()?.to_lowercase().as_str() {
        return None;
    }
    let value = split.get(1)?.parse::<T>().ok()?;
    Some(value)
}
fn parse_ll(line: Option<Result<String, Error>>) -> Option<(CornerType, f64)> {
    let line = line.and_then(Result::ok)?;
    let split = line.split_whitespace().collect::<Vec<&str>>();
    let corner_type = CornerType::from_str(split.first()?)?;
    let value = split.get(1)?.parse::<f64>().ok()?;
    Some((corner_type, value))
}
