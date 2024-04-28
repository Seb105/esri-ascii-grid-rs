use std::{
    io::{self, BufRead, BufReader, Lines, Read, Seek, SeekFrom},
    vec::IntoIter,
};

use replace_with::replace_with_or_abort;

use crate::{error::Error, header::EsriASCIIRasterHeader};

struct LineSeeker {
    line: usize,
    position: u64,
}
impl LineSeeker {
    fn update(&mut self, line: usize, position: u64) {
        self.line = line;
        self.position = position;
    }
}

pub struct EsriASCIIReader<R> {
    pub header: EsriASCIIRasterHeader,
    reader: BufReader<R>,
    line_cache: Vec<Option<Vec<f64>>>,
    line_start_cache: Vec<Option<u64>>,
    data_start: u64,
    line_seeker: LineSeeker,
}
impl<R: Read + Seek> EsriASCIIReader<R> {
    /// Create a new `EsriASCIIReader` from a file.
    ///
    /// When creating the file, only the header is read.
    ///
    /// # Examples
    /// ```rust
    /// use esri_ascii_grid::ascii_file::EsriASCIIReader;
    /// let file = std::fs::File::open("test_data/test.asc").unwrap();
    /// let mut grid = EsriASCIIReader::from_file(file).unwrap();
    /// // Indexing the file is optional, but is recommended if you are going to be repeatedly calling any `get` function
    /// // This will build the index and cache the file positions of each line, it will take a while for large files
    /// // but will drastically increase the speed subsequent `get` calls.
    /// grid.build_index().unwrap();
    /// // Spot check a few values
    /// assert_eq!(grid.get(390000.0, 344000.0).unwrap(), 141.2700042724609375);
    /// assert_eq!(grid.get(390003.0, 344003.0).unwrap(), 135.44000244140625);
    /// ```
    /// # Errors
    /// Returns an error if there is something wrong with the header, such as missing values
    /// The error should give a description of the problem.
    pub fn from_file(file: R) -> Result<Self, crate::error::Error> {
        let mut reader = BufReader::new(file);
        let grid_header = EsriASCIIRasterHeader::from_reader(&mut reader)?;
        let data_start = reader.stream_position()?;
        let mut line_start_cache = vec![None; grid_header.num_rows()];
        line_start_cache[grid_header.num_rows() - 1] = Some(data_start);
        Ok(Self {
            header: grid_header,
            reader,
            line_cache: vec![None; grid_header.num_rows()],
            line_start_cache: line_start_cache,
            data_start,
            line_seeker: LineSeeker {
                line: grid_header.num_rows() - 1,
                position: data_start,
            },
        })
    }
    /// Build an index of the file.
    /// This will take a while for very large files, but will make subsequent calls to `get` or any such function much faster.
    /// If you are going to be repeatedly calling and `get` on a big file it is recommended to call this function first.
    ///
    /// # Errors
    /// Returns an error if there is some problem with the indexing, such as the file being too short.
    pub fn build_index(&mut self) -> Result<(), crate::error::Error> {
        // Clear any existing cache
        self.line_start_cache = vec![None; self.header.num_rows()];
        let num_rows = self.header.num_rows();
        let reader = self.reader.by_ref();
        reader.seek(SeekFrom::Start(self.data_start))?;
        for row in (0..num_rows).rev() {
            self.line_start_cache[row] = Some(reader.stream_position()?);
            reader
                .lines()
                .next()
                .ok_or_else(|| crate::error::Error::MismatchedRowCount(num_rows, row))??;
        }
        Ok(())
    }
    /// Returns the value at the given row and column.
    /// 0, 0 is the bottom left corner. The row and column are zero indexed.
    /// # Examples
    /// ```rust
    /// use esri_ascii_grid::ascii_file::EsriASCIIReader;
    /// let file = std::fs::File::open("test_data/test.asc").unwrap();
    /// let mut grid = EsriASCIIReader::from_file(file).unwrap();
    /// // Indexing the file is optional, but is recommended if you are going to be repeatedly calling any `get` function
    /// // This will build the index and cache the file positions of each line, it will take a while for large files
    /// // but will drastically increase the speed subsequent `get` calls.
    /// grid.build_index().unwrap();
    /// // Spot check a few values
    /// assert_eq!(grid.get_index(0, 0).unwrap(), 141.270_004_272_460_937_5);
    /// assert_eq!(grid.get_index(3, 3).unwrap(), 135.440_002_441_406_25);
    /// ```
    ///
    /// # Errors
    /// Returns an error if the row or column is out of bounds or is not a valid number.
    ///
    /// # Panics
    /// Panics if the row or column is out of bounds, which should not happen as they are checked in this function.
    pub fn get_index(&mut self, row: usize, col: usize) -> Result<f64, crate::error::Error> {
        if row >= self.header.nrows || col >= self.header.ncols {
            Err(crate::error::Error::OutOfBounds(row, col))?
        };
        if let Some(values) = &self.line_cache[row] {
            let val = values[col];
            return Ok(val);
        }
        let reader = self.reader.by_ref();
        let line = if let Some(line_pos) = self.line_start_cache[row] {
            reader.seek(SeekFrom::Start(line_pos))?;
            reader.lines().next().unwrap()?
        } else {
            seek_to_line(reader, row, &mut self.line_seeker, &mut self.line_start_cache)?;
            reader.lines().next().unwrap()?
        };
        let values: Vec<f64> = line
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        let ret = values[col];
        self.line_cache[row] = Some(values);
        Ok(ret)
    }
    /// Returns the value at the given x and y coordinates.
    ///
    ///
    /// If the coordinates are outside the bounds of the raster, nothing is returned.
    ///
    /// If the coordinates are within the bounds of the raster, but not on a cell, the behaviour depends on the `corner_type` of the raster.
    /// If the `corner_type` is `Corner`, the value at the bottom left corner of the cell is returned.
    /// If the `corner_type` is `Center`, the value at the center of the cell is returned.
    ///
    /// # Examples
    /// ```rust
    /// use esri_ascii_grid::ascii_file::EsriASCIIReader;
    /// let file = std::fs::File::open("test_data/test.asc").unwrap();
    /// let mut grid = EsriASCIIReader::from_file(file).unwrap();
    /// // Indexing the file is optional, but is recommended if you are going to be repeatedly calling any `get` function
    /// // This will build the index and cache the file positions of each line, it will take a while for large files
    /// // but will drastically increase the speed subsequent `get` calls.
    /// grid.build_index().unwrap();
    /// // Spot check a few values
    /// assert_eq!(grid.get(390000.0, 344000.0).unwrap(), 141.2700042724609375);
    /// assert_eq!(grid.get(390003.0, 344003.0).unwrap(), 135.44000244140625);
    /// ```
    ///
    /// # Panics
    /// Panics if the coordinates are outside the bounds of the raster, which should not happen as they are checked in this function.
    pub fn get(&mut self, x: f64, y: f64) -> Option<f64> {
        let (col, row) = self.header.index_of(x, y)?;
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
    /// # Examples
    /// ```rust
    /// use esri_ascii_grid::ascii_file::EsriASCIIReader;
    /// let file = std::fs::File::open("test_data/test.asc").unwrap();
    /// let mut grid = EsriASCIIReader::from_file(file).unwrap();
    /// // Indexing the file is optional, but is recommended if you are going to be repeatedly calling any `get` function
    /// // This will build the index and cache the file positions of each line, it will take a while for large files
    /// // but will drastically increase the speed subsequent `get` calls.
    /// grid.build_index().unwrap();
    /// // Spot check a few values
    /// assert_eq!(grid.get_interpolate(390000.0, 344000.0).unwrap(), 141.2700042724609375);
    /// assert_eq!(grid.get_interpolate(390003.0, 344003.0).unwrap(), 135.44000244140625);
    /// ```
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
    type Item = Result<(usize, usize, f64), Error>;
    type IntoIter = EsriASCIIRasterIntoIterator<R>;
    /// Returns an iterator over the values in the raster.
    /// The iterator will scan the raster from left to right, top to bottom.
    /// So the row will start at num_rows-1 and decrease to 0.
    /// The column will start at 0 and increase to num_cols-1.
    ///
    /// If an error is encountered at any point, the iterator will return an
    /// `Err` once and halt.
    ///
    /// ```rust
    /// let file = std::fs::File::open("test_data/test.asc").unwrap();
    /// let grid = esri_ascii_grid::ascii_file::EsriASCIIReader::from_file(file).unwrap();
    /// let grid_size = grid.header.num_rows() * grid.header.num_cols();
    /// let header = grid.header;
    /// let iter = grid.into_iter();
    /// let mut num_elements = 0;
    /// for cell in iter {
    ///     let Ok((row, col, value)) = cell else {
    ///         panic!("your error handler")
    ///     };
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
        let line_reader = LineReader::Uninitialized {
            data_start: self.data_start,
            reader: self.reader,
        };

        EsriASCIIRasterIntoIterator {
            header: self.header,
            line_reader,
            row_it: None,
            row: 0,
            col: 0,
            terminated: false,
        }
    }
}
fn seek_to_line<R: Read + Seek> (reader: &mut BufReader<R>, row: usize, line_seeker: &mut LineSeeker, line_start_cache: &mut Vec<Option<u64>>) -> Result<(), Error> {
    let latest_line = line_seeker.line;
    let latest_pos = line_seeker.position;
    reader.seek(SeekFrom::Start(latest_pos))?;
    for i in (row..latest_line).rev() {
        println!("seeking to line {}", i);
        reader
            .lines()
            .next()
            .ok_or_else(|| Error::MismatchedRowCount(row, i))??;
        line_start_cache[i] = Some(reader.stream_position()?);

    }
    line_seeker.update(row, reader.stream_position()?);
    Ok(())
}


enum LineReader<R> {
    Uninitialized {
        data_start: u64,
        reader: BufReader<R>,
    },
    Initialized {
        lines: Lines<BufReader<R>>,
    },
    /// Will reach this state if an error occurs during initialization.
    Invalid {
        /// Temporary storage for the error.
        error: Option<io::Error>,
    },
}
impl<R: Read + Seek> Iterator for LineReader<R> {
    type Item = Result<String, io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // try to initialize
        if matches!(self, Self::Uninitialized { .. }) {
            replace_with_or_abort(self, |r| {
                let Self::Uninitialized {
                    data_start,
                    mut reader,
                } = r
                else {
                    unreachable!()
                };
                let convert = move || -> Result<Lines<BufReader<R>>, io::Error> {
                    reader.seek(SeekFrom::Start(data_start))?;
                    Ok(reader.lines())
                };
                match convert() {
                    Ok(lines) => Self::Initialized { lines },
                    Err(err) => Self::Invalid { error: Some(err) },
                }
            });
            if let Self::Invalid { error } = self {
                let error = error.take().unwrap();
                return Some(Err(error));
            }
        }

        match self {
            Self::Uninitialized { .. } => unreachable!(),
            Self::Invalid { .. } => {
                // error has been returned for the previous iteration, so we halt here
                return None;
            }
            Self::Initialized { lines } => lines.next(),
        }
    }
}

pub struct EsriASCIIRasterIntoIterator<R> {
    pub header: EsriASCIIRasterHeader,
    line_reader: LineReader<R>,
    row_it: Option<IntoIter<f64>>,
    row: usize,
    col: usize,
    terminated: bool,
}
impl<R: Read + Seek> Iterator for EsriASCIIRasterIntoIterator<R> {
    type Item = Result<(usize, usize, f64), Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.terminated {
            return None;
        }

        // we check this first because row_it is initialized as None
        // we don't want to increment row index on initial pass
        if self.col >= self.header.ncols {
            // discard current row and set indices for next row
            let _ = self.row_it.take();
            self.row += 1;
            self.col = 0;
            if self.row >= self.header.nrows {
                self.terminated = true;
                return None;
            }
        }

        // load new row
        if self.row_it.is_none() {
            match self.line_reader.next() {
                Some(Ok(line)) => {
                    match line
                        .split_whitespace()
                        .map(|s| s.parse::<f64>())
                        .collect::<Result<Vec<_>, _>>()
                    {
                        Ok(row) => self.row_it = Some(row.into_iter()),
                        Err(error) => {
                            self.terminated = true;
                            Some(Result::<(usize, usize, f64), Error>::Err(error.into()));
                        }
                    }
                }
                Some(Err(error)) => {
                    self.terminated = true;
                    return Some(Err(error.into()));
                }
                None => {
                    self.terminated = true;
                    return None;
                }
            }
        }

        let current_col = self.col;
        let current_row = self.row;

        // row_it is guaranteed to be Some here
        let Some(value) = self.row_it.as_mut().unwrap().next() else {
            return Some(Err(Error::MismatchColumnCount(self.header.ncols, self.col)));
        };
        self.col += 1;

        Some(Ok((
            self.header.nrows - 1 - current_row,
            current_col,
            value,
        )))
    }
}
