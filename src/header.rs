use std::{
    io::{BufRead, BufReader, Error, Read, Seek},
    str::FromStr,
};

// use serde::{Serialize, Deserialize};


#[derive(Debug)]
pub struct EsriASCIIRasterHeader {
    pub ncols: usize,
    pub nrows: usize,
    pub xll: f64,
    pub yll: f64,
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
        let ncols = parse_header_line::<usize>(lines.next(), "ncols").ok_or_else(
            || Error::new(std::io::ErrorKind::InvalidData, "ncols invalid"),
        )?;
        let nrows = parse_header_line::<usize>(lines.next(), "nrows").ok_or_else(
            || Error::new(std::io::ErrorKind::InvalidData, "nrows invalid"),
        )?;
        let (corner_type, xll) = parse_ll(lines.next()).ok_or_else(
            || Error::new(std::io::ErrorKind::InvalidData, "xll invalid"),
        )?;
        let (_, yll) = parse_ll(lines.next()).ok_or_else(
            || Error::new(std::io::ErrorKind::InvalidData, "yll invalid"),
        )?;
        let cellsize = parse_header_line::<f64>(lines.next(), "cellsize").ok_or_else(
            || Error::new(std::io::ErrorKind::InvalidData, "cellsize invalid"),
        )?;
        let nodata_value = parse_header_line::<f64>(lines.next(), "nodata_value");
        Ok(Self {
            ncols,
            nrows,
            xll,
            yll,
            cornertype: corner_type,
            cellsize,
            nodata_value,
        })
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
fn parse_header_line<T: FromStr>(
    line: Option<Result<String, Error>>,
    expected: &str,
) -> Option<T> {
    let line = line.and_then(|line| line.ok())?;
    let split = line.split_whitespace().collect::<Vec<&str>>();
    if expected != split.first()?.to_lowercase().as_str() {
        return None;
    }
    let value = split.get(1)?.parse::<T>().ok()?;
    Some(value)
}
fn parse_ll(line: Option<Result<String, Error>>) -> Option<(CornerType, f64)> {
    let line = line.and_then(|line| line.ok())?;
    let split = line.split_whitespace().collect::<Vec<&str>>();
    let corner_type = CornerType::from_str(split.first()?)?;
    let value = split.get(1)?.parse::<f64>().ok()?;
    Some((corner_type, value))
}
