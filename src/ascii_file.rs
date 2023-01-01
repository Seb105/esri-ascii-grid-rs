use std::{
    io::{BufRead, BufReader, Error, Lines, Read, Seek},
    vec::IntoIter,
};

use crate::header::{EsriASCIIRasterHeader};

pub struct EsriASCIIReader<R> {
    pub header: EsriASCIIRasterHeader,
    reader: BufReader<R>,
    data_start: u64,
}
impl<R: Read + Seek> EsriASCIIReader<R> {
    /// Create a new `EsriASCIIReader` from a file.
    ///
    /// When creating the file, only the header is read.
    ///
    /// # Examples
    /// ```rust
    /// use esri_ascii_grid_rs::ascii_file::EsriASCIIReader;
    /// let file = std::fs::File::open("test_data/test.asc").unwrap();
    /// let mut grid = EsriASCIIReader::from_file(file).unwrap();
    ///
    /// // Spot check a few values
    /// assert_eq!(grid.get(390000.0, 344000.0).unwrap(), 141.2700042724609375);
    /// assert_eq!(grid.get(390003.0, 344003.0).unwrap(), 135.44000244140625);
    /// ```
    /// # Errors
    /// Returns an IO error if there is someghing wrong with the header, such as missing values
    /// The IO error should give a description of the problem.
    pub fn from_file(file: R) -> Result<Self, Error> {
        let mut reader = BufReader::new(file);
        let grid_header = EsriASCIIRasterHeader::from_reader(&mut reader)?;
        let data_start = reader.stream_position()?;
        Ok(Self {
            header: grid_header,
            reader,
            data_start,
        })
    }
    /// Returns the value at the given row and column.
    /// 0, 0 is the bottom left corner. The row and column are zero indexed.
    /// 
    /// # Errors
    /// Returns an IO error if the row or column is out of bounds or is not a valid number.
    pub fn get_index(&mut self, row: usize, col: usize) -> Result<f64, Error> {
        if row >= self.header.nrows || col >= self.header.ncols {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "Index out of bounds",
            ));
        }
        let num_rows = self.header.num_rows();
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
    /// If the coordinates are within the bounds of the raster, but not on a cell, the value of the nearest cell is returned
    /// 
    /// # Panics
    /// Panics if the coordinates are outside the bounds of the raster, which should not happen as they are checked in this function.
    pub fn get(&mut self, x: f64, y: f64) -> Option<f64> {
        let (row, col) = self.header.index_of(x, y)?;
        let val = self.get_index(row, col).unwrap();
        Some(val)
    }
    /// Returns the value at the given x and y coordinates.
    ///
    /// If the coordinates are outside the bounds of the raster, nothing is returned.
    ///
    /// The value is interpolated from the four nearest cells.
    ///
    /// Even if the coordinates are exactly on a cell, the value is interpolated and so may or may not be the same as the value at the cell due to floating point errors.
    /// 
    /// # Panics
    /// Panics if the coordinates are outside the bounds of the raster, which should not happen as they are checked in this function.
    pub fn get_interpolate(&mut self, x: f64, y: f64) -> Option<f64> {
        if x < self.header.min_x()
            || x > self.header.max_x()
            || y < self.header.min_y()
            || y > self.header.max_y()
        {
            return None;
        }
        let ll_col = (((x - self.header.min_x()) / self.header.cellsize).floor() as usize)
            .min(self.header.ncols - 2);
        let ll_row = (((y - self.header.min_y()) / self.header.cellsize).floor() as usize)
            .min(self.header.nrows - 2);

        let (ll_x, ll_y) = self.header.index_pos(ll_row, ll_col).unwrap();

        let ll = self.get_index(ll_row, ll_col).unwrap();
        let lr = self.get_index(ll_row, ll_col + 1).unwrap();
        let ul = self.get_index(ll_row + 1, ll_col).unwrap();
        let ur = self.get_index(ll_row + 1, ll_col + 1).unwrap();

        let vert_weight = (x - ll_x) / self.header.cell_size();
        let horiz_weight = (y - ll_y) / self.header.cell_size();

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
    /// let grid_size = grid.header.num_rows() * grid.header.num_cols();
    /// let header = grid.header;
    /// let iter = grid.into_iter();
    /// let mut num_elements = 0;
    /// for (row, col, value) in iter {
    ///     num_elements += 1;
    ///     if row == 3 && col == 3 {
    ///         let (x, y) = header.index_pos(col, row).unwrap();
    ///         assert_eq!(x, 390003.0);
    ///         assert_eq!(y, 344003.0);
    ///         assert_eq!(value, 135.44000244140625);
    ///     }
    ///     if row == 0 && col == 0 {
    ///         let (x, y) = header.index_pos(col, row).unwrap();
    ///         assert_eq!(x, 390000.0);
    ///         assert_eq!(y, 344000.0);
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
    pub header: EsriASCIIRasterHeader,
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
