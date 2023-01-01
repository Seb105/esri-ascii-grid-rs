use std::{
    io::{BufRead, BufReader, Error, Lines, Read, Seek},
    vec::IntoIter
};

use crate::header::{CornerType, EsriASCIIRasterHeader};

pub struct EsriASCIIReader<R> {
    pub header: EsriASCIIRasterHeader,
    reader: BufReader<R>,
    data_start: u64,
}
impl<R: Read + Seek> EsriASCIIReader<R> {
    /// Create a new EsriASCIIReader from a file.
    ///
    /// When creating the file, only the header is read.
    /// 
    /// 
    /// ```rust
    /// use esri_ascii_grid_rs::ascii_file::EsriASCIIReader;
    /// let file = std::fs::File::open("test_data/test.asc").unwrap();
    /// let mut grid = EsriASCIIReader::from_file(file).unwrap();
    ///
    /// // Spot check a few values
    /// assert_eq!(grid.get(390000.0, 344000.0).unwrap(), 141.2700042724609375);
    /// assert_eq!(grid.get(390003.0, 344003.0).unwrap(), 135.44000244140625);
    /// ```
    ///
    pub fn from_file(file: R) -> Result<Self, Error> {
        let mut reader = BufReader::new(file);
        let header = EsriASCIIRasterHeader::from_reader(&mut reader)?;
        let data_start = reader.stream_position()?;
        Ok(Self {
            header,
            reader,
            data_start,
        })
    }
    pub fn num_rows(&self) -> usize {
        self.header.nrows
    }
    pub fn num_cols(&self) -> usize {
        self.header.ncols
    }
    pub fn min_x(&self) -> f64 {
        self.header.xll
    }
    pub fn max_x(&self) -> f64 {
        self.min_x() + self.header.cellsize * (self.header.ncols - 1) as f64
    }
    pub fn min_y(&self) -> f64 {
        self.header.yll
    }
    pub fn max_y(&self) -> f64 {
        self.min_y() + self.header.cellsize * (self.header.nrows - 1) as f64
    }
    pub fn cell_size(&self) -> f64 {
        self.header.cellsize
    }
    pub fn no_data_value(&self) -> Option<f64> {
        self.header.nodata_value
    }
    /// ESRI ASCII files can have either a corner or centre cell type.
    ///
    /// If the cell type is corner, the values are the at coordinates of the bottom left corner of the cell.
    ///
    /// If the cell type is centre, the values are the at coordinates of the centre of the cell.
    pub fn corner_type(&self) -> CornerType {
        self.header.cornertype
    }
    /// Returns the value at the given row and column.
    /// 0, 0 is the bottom left corner. The row and column are zero indexed.
    pub fn get_index(&mut self, row: usize, col: usize) -> Result<f64, Error> {
        if row >= self.header.nrows || col >= self.header.ncols {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "Index out of bounds",
            ));
        }
        let num_rows = self.num_rows();
        let reader = self.reader.by_ref();
        reader.rewind()?;
        reader.seek(std::io::SeekFrom::Start(self.data_start))?;
        let mut lines = reader.lines();
        let line = lines.nth(num_rows - 1 - row).ok_or_else(|| {
            Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid row at {row}"),
            )
        })??;
        let value = line
            .split_whitespace()
            .nth(col)
            .ok_or_else(|| {
                Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid row, column at Row: {row} Col :{col}"),
                )
            })?
            .parse::<f64>()
            .map_err(|err| Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
        Ok(value)
    }
    /// Returns the value at the given x and y coordinates.
    ///
    ///
    /// If the coordinates are outside the bounds of the raster, nothing is returned.
    ///
    /// If the coordinates are within the bounds of the raster, but not on a cell, the value of the nearest cell is returned.
    pub fn get(&mut self, x: f64, y: f64) -> Option<f64> {
        let (row, col) = self.index_of(x, y)?;
        let val = self.get_index(row, col).unwrap();
        Some(val)
    }
    /// Get the x and y coordinates of the cell at the given row and column, or nothing if it is out of bounds.
    pub fn index_pos(&self, row: usize, col: usize) -> Option<(f64, f64)> {
        let nrows = self.header.nrows;
        let ncols = self.header.ncols;
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
        let col = ((x - self.min_x()) / self.header.cellsize).round() as usize;
        let row = ((y - self.min_y()) / self.header.cellsize).round() as usize;
        Some((col, row))
    }
    /// Returns the value at the given x and y coordinates.
    ///
    /// If the coordinates are outside the bounds of the raster, nothing is returned.
    ///
    /// The value is interpolated from the four nearest cells.
    ///
    /// Even if the coordinates are exactly on a cell, the value is interpolated and so may or may not be the same as the value at the cell due to floating point errors.
    pub fn get_interpolate(&mut self, x: f64, y: f64) -> Option<f64> {
        if x < self.min_x() || x > self.max_x() || y < self.min_y() || y > self.max_y() {
            return None;
        }
        let ll_col = (((x - self.min_x()) / self.header.cellsize).floor() as usize)
            .min(self.header.ncols - 2);
        let ll_row = (((y - self.min_y()) / self.header.cellsize).floor() as usize)
            .min(self.header.nrows - 2);

        let (ll_x, ll_y) = self.index_pos(ll_row, ll_col).unwrap();

        let ll = self.get_index(ll_row, ll_col).unwrap();
        let lr = self.get_index(ll_row, ll_col + 1).unwrap();
        let ul = self.get_index(ll_row + 1, ll_col).unwrap();
        let ur = self.get_index(ll_row + 1, ll_col + 1).unwrap();

        let vert_weight = (x - ll_x) / self.cell_size();
        let horiz_weight = (y - ll_y) / self.cell_size();

        let ll_weight = (1.0 - vert_weight) * (1.0 - horiz_weight);
        let ur_weight = vert_weight * horiz_weight;
        let ul_weight = (1.0 - vert_weight) * horiz_weight;
        let lr_weight = vert_weight * (1.0 - horiz_weight);

        let value = ul * ul_weight + ur * ur_weight + ll * ll_weight + lr * lr_weight;
        Some(value)
    }
}
impl<R: Read + Seek> IntoIterator for EsriASCIIReader<R> {
    type Item = (usize, usize, f64);
    type IntoIter = EsriASCIIRasterIntoIterator<R>;
    /// Returns an iterator over the values in the raster.
    /// The iterator will scan the raster from left to right, top to bottom.
    /// So the row will start at num_rows-1 and decrease to 0.
    /// The column will start at 0 and increase to num_cols-1.
    /// 
    /// ```rust
    /// let file = std::fs::File::open("test_data/test.asc").unwrap();
    /// let grid = esri_ascii_grid_rs::ascii_file::EsriASCIIReader::from_file(file).unwrap();
    /// let grid_size = grid.num_rows() * grid.num_cols();
    /// let iter = grid.into_iter();
    /// let mut num_elements = 0;
    /// for (row, col, value) in iter {
    ///     num_elements += 1;
    ///     if row == 3 && col == 3 {
    ///         let (x, y) = grid.index_pos(row, col).unwrap();
    ///         assert_eq!(value, 135.44000244140625);
    ///     }
    ///     if row == 0 && col == 0 {
    ///         assert_eq!(value, 141.2700042724609375);
    ///     }
    /// }
    /// assert_eq!(grid_size, num_elements);
    /// ```
    /// 
    fn into_iter(self) -> Self::IntoIter {
        let mut reader = self.reader;
        reader.rewind().unwrap();
        reader
            .seek(std::io::SeekFrom::Start(self.data_start))
            .unwrap();
        let mut lines = reader.lines();
        let line_string = lines.next().unwrap().unwrap();
        let line = line_string
            .split_whitespace()
            .map(|s| s.parse::<f64>().unwrap())
            .collect::<Vec<f64>>()
            .into_iter();
        EsriASCIIRasterIntoIterator {
            header: self.header,
            lines,
            line,
            row: 0,
            col: 0,
        }
    }
}

pub struct EsriASCIIRasterIntoIterator<R> {
    header: EsriASCIIRasterHeader,
    lines: Lines<BufReader<R>>,
    line: IntoIter<f64>,
    row: usize,
    col: usize,
}
impl<R: Read + Seek> Iterator for EsriASCIIRasterIntoIterator<R> {
    type Item = (usize, usize, f64);
    fn next(&mut self) -> Option<Self::Item> {
        if self.col >= self.header.ncols {
            self.row += 1;
            self.col = 0;
            if self.row >= self.header.nrows {
                return None;
            }
            let line_string = self.lines.next().unwrap().unwrap();
            let line = line_string
                .split_whitespace()
                .map(|s| s.parse::<f64>().unwrap())
                .collect::<Vec<f64>>()
                .into_iter();
            self.line = line;
        }
        let current_col = self.col;
        let current_row = self.row;
        self.col += 1;
        let value = self.line.next().unwrap();
        Some((self.header.nrows - 1 - current_row, current_col, value))
    }
}
