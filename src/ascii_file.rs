use std::{
    io::{self, BufRead, BufReader, Lines, Read, Seek, SeekFrom},
    vec::IntoIter,
};

use num_traits::NumCast;
use replace_with::replace_with_or_abort;

use crate::{
    error::{self, Error},
    header::{EsriASCIIRasterHeader, Numerical},
};

#[derive(Debug)]
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

/// A reader for ESRI ASCII raster files.
/// This reader reads the header of the file and then reads the data on demand.
/// The data is cached in memory, so that the file is only read once.
///
/// # Type Parameters
/// * `R` - The type of the file. This should be a file that implements `Read` and `Seek`.
/// * `T` - The type of the coordinates. Should be a number.
/// * `U` - The type of the height values in the grid. Should be a number
#[derive(Debug)]
pub struct EsriASCIIReader<R, T: Numerical, U: Numerical> {
    pub header: EsriASCIIRasterHeader<T, U>,
    reader: BufReader<R>,
    line_cache: Vec<Option<Vec<U>>>,
    line_start_cache: Vec<Option<u64>>,
    data_start: u64,
    line_seeker: LineSeeker,
}
impl<R, T, U> EsriASCIIReader<R, T, U>
where
    R: Read + Seek,
    T: Numerical,
    error::Error: From<<T as Numerical>::Err>,
    U: Numerical,
    error::Error: From<<U as Numerical>::Err>,
{
    /// Create a new `EsriASCIIReader` from a file.
    ///
    /// When creating the file, only the header is read at first.
    ///
    /// # Type Parameters
    /// * `R` - The type of the file. This should be a file that implements `Read` and `Seek`.
    /// * `T` - The type of the coordinates. Should be a number.
    /// * `U` - The type of the height values in the grid. Should be a number
    ///
    /// # Examples
    /// ```rust
    /// use esri_ascii_grid::ascii_file::EsriASCIIReader;
    /// use std::fs::File;
    /// let file = File::open("test_data/test.asc").unwrap();
    /// let mut grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();
    /// // Spot check a few values
    /// assert_eq!(grid.get(390000.0, 344000.0).unwrap(), 141.2700042724609375);
    /// assert_eq!(grid.get(390003.0, 344003.0).unwrap(), 135.44000244140625);
    /// ```
    /// # Errors
    /// Returns an error if there is something wrong with the header, such as missing values
    /// The error should give a description of the problem..
    pub fn from_file(file: R) -> Result<Self, crate::error::Error> {
        let mut reader = BufReader::new(file);
        let grid_header = EsriASCIIRasterHeader::from_reader(&mut reader)?;
        let data_start = reader.stream_position()?;
        let mut line_start_cache = vec![None; grid_header.num_rows()];
        line_start_cache[0] = Some(data_start);
        Ok(Self {
            header: grid_header,
            reader,
            line_cache: vec![None; grid_header.num_rows()],
            line_start_cache,
            data_start,
            line_seeker: LineSeeker {
                line: 0,
                position: data_start,
            },
        })
    }
    /// Returns the value at the given row and column.
    /// 0, 0 is the top left corner. The row and column are zero indexed.
    /// # Examples
    /// ```rust
    /// use std::fs::File;
    /// use esri_ascii_grid::ascii_file::EsriASCIIReader;
    /// let file = File::open("test_data/test.asc").unwrap();
    /// let mut grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();
    /// // Spot check a few values
    /// assert_eq!(grid.get_index(999, 0).unwrap(), 141.270_004_272_460_937_5);
    /// assert_eq!(grid.get_index(996, 3).unwrap(), 135.440_002_441_406_25);
    /// ```
    ///
    /// # Errors
    /// Returns an error if the row or column is out of bounds or is not a valid number.
    ///
    /// # Panics
    /// Panics if the row or column is out of bounds, which should not happen as they are checked in this function.
    pub fn get_index(&mut self, row: usize, col: usize) -> Result<U, crate::error::Error> {
        if row >= self.header.nrows || col >= self.header.ncols {
            Err(crate::error::Error::OutOfBounds(row, col))?;
        }
        if let Some(values) = &self.line_cache[row] {
            let val = values[col];
            return Ok(val);
        }
        let reader = self.reader.by_ref();
        let line = if let Some(line_pos) = self.line_start_cache[row] {
            reader.seek(SeekFrom::Start(line_pos))?;
            reader.lines().next().unwrap()?
        } else {
            seek_to_line(
                reader,
                row,
                &mut self.line_seeker,
                &mut self.line_start_cache,
            )?;
            reader.lines().next().unwrap()?
        };
        let value_res = line
            .split_whitespace()
            .map(|s| s.parse::<U>().map_err(|_| Error::TypeCast(
                format!("{row}, {col}"),
                "grid value".to_owned(),
                std::any::type_name::<U>(),
            )));
        let values: Vec<U> = value_res.collect::<Result<Vec<U>, Error>>()?;
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
    /// use std::fs::File;
    /// let file = File::open("test_data/test.asc").unwrap();
    /// let mut grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();
    /// // Spot check a few values
    /// assert_eq!(grid.get(390000.0, 344000.0).unwrap(), 141.2700042724609375);
    /// assert_eq!(grid.get(390003.0, 344003.0).unwrap(), 135.44000244140625);
    /// ```
    ///
    /// # Panics
    /// Panics if the coordinates are outside the bounds of the raster, which should not happen as they are checked in this function.
    pub fn get(&mut self, x: T, y: T) -> Option<U> {
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
    /// # Examples
    /// ```rust
    /// use esri_ascii_grid::ascii_file::EsriASCIIReader;
    /// use std::fs::File;
    /// let file = File::open("test_data/test.asc").unwrap();
    /// let mut grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();;
    /// // Spot check a few values
    /// assert_eq!(grid.get_interpolate(390000.0, 344000.0).unwrap(), 141.2700042724609375);
    /// assert_eq!(grid.get_interpolate(390003.0, 344003.0).unwrap(), 135.44000244140625);
    /// ```
    ///
    /// # Panics
    /// Panics if the coordinates are outside the bounds of the raster, which should not happen as they are checked in this function.
    pub fn get_interpolate(&mut self, x: T, y: T) -> Option<U> {
        if x < self.header.min_x()
            || x > self.header.max_x()
            || y < self.header.min_y()
            || y > self.header.max_y()
        {
            return None;
        }
        let (mut ll_row, mut ll_col) = self.header.index_of(x, y).unwrap();
        ll_col = ll_col.min(self.header.num_cols() - 2);
        ll_row = ll_row.max(1);

        let (ll_x, ll_y) = self.header.index_pos(ll_row, ll_col).unwrap();

        let ll = <f64 as NumCast>::from(self.get_index(ll_row, ll_col).unwrap()).unwrap();
        let lr = <f64 as NumCast>::from(self.get_index(ll_row, ll_col + 1).unwrap()).unwrap();
        let ul = <f64 as NumCast>::from(self.get_index(ll_row - 1, ll_col).unwrap()).unwrap();
        let ur = <f64 as NumCast>::from(self.get_index(ll_row - 1, ll_col + 1).unwrap()).unwrap();

        let cell_size = <f64 as NumCast>::from(self.header.cell_size()).unwrap();
        let vert_weight = <f64 as NumCast>::from(x - ll_x).unwrap() / cell_size;
        let horiz_weight = <f64 as NumCast>::from(y - ll_y).unwrap() / cell_size;

        let ll_weight = (1.0 - vert_weight) * (1.0 - horiz_weight);
        let ur_weight = vert_weight * horiz_weight;
        let ul_weight = (1.0 - vert_weight) * horiz_weight;
        let lr_weight = vert_weight * (1.0 - horiz_weight);

        let value: f64 = ul * ul_weight + ur * ur_weight + ll * ll_weight + lr * lr_weight;
        Some(U::from(value).unwrap())
    }
}
impl<R, T, U> IntoIterator for EsriASCIIReader<R, T, U>
where
    R: Read + Seek,
    T: Numerical,
    error::Error: From<<T as Numerical>::Err>,
    U: Numerical,
    error::Error: From<<U as Numerical>::Err>,
{
    type Item = Result<(usize, usize, U), Error>;
    type IntoIter = EsriASCIIRasterIntoIterator<R, T, U>;
    /// Returns an iterator over the values in the raster.
    /// The iterator will scan the raster from left to right, top to bottom.
    /// Row 0 is the top row
    /// Column 0 is the leftmost column
    ///
    /// If an error is encountered at any point, the iterator will return an
    /// `Err` once and halt.
    ///
    /// ```rust
    /// use esri_ascii_grid::ascii_file::EsriASCIIReader;
    /// use std::fs::File;
    /// let file = File::open("test_data/test.asc").unwrap();
    /// let grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();
    /// let grid_size = grid.header.num_rows() * grid.header.num_cols();
    /// let header = grid.header;
    /// let iter = grid.into_iter();
    /// let mut num_elements = 0;
    /// for cell in iter {
    ///     let Ok((row, col, value)) = cell else {
    ///         panic!("your error handler")
    ///     };
    ///     num_elements += 1;
    ///     if row == 996 && col == 3 {
    ///         let (x, y) = header.index_pos(row, col).unwrap();
    ///         assert_eq!(x, 390003.0);
    ///         assert_eq!(y, 344003.0);
    ///         assert_eq!(value, 135.44000244140625);
    ///     }
    ///     if row == header.nrows-1 && col == 0 {
    ///         let (x, y) = header.index_pos(row, col).unwrap();
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
fn seek_to_line<R: Read + Seek>(
    reader: &mut BufReader<R>,
    row: usize,
    line_seeker: &mut LineSeeker,
    line_start_cache: &mut [Option<u64>],
) -> Result<(), Error> {
    let latest_line = line_seeker.line;
    let latest_pos = line_seeker.position;
    reader.seek(SeekFrom::Start(latest_pos))?;
    for (cache, line) in line_start_cache[latest_line..row].iter_mut().zip(latest_line..) {
        *cache = Some(reader.stream_position()?);
        reader
            .lines()
            .next()
            .ok_or(Error::MismatchedRowCount(row, line))??;
    }
    line_seeker.update(row, reader.stream_position()?);
    Ok(())
}

#[derive(Debug)]
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
                None
            }
            Self::Initialized { lines } => lines.next(),
        }
    }
}

#[derive(Debug)]
pub struct EsriASCIIRasterIntoIterator<R, T: Numerical, U: Numerical> {
    pub header: EsriASCIIRasterHeader<T, U>,
    line_reader: LineReader<R>,
    row_it: Option<IntoIter<U>>,
    row: usize,
    col: usize,
    terminated: bool,
}
impl<R, T, U> Iterator for EsriASCIIRasterIntoIterator<R, T, U>
where
    R: Read + Seek,
    T: Numerical,
    error::Error: From<<T as Numerical>::Err>,
    U: Numerical,
    error::Error: From<<U as Numerical>::Err>,
{
    type Item = Result<(usize, usize, U), Error>;
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
                        .map(str::parse)
                        .collect::<Result<Vec<_>, _>>()
                    {
                        Ok(row) => self.row_it = Some(row.into_iter()),
                        Err(error) => {
                            self.terminated = true;
                            let _ = Result::<(usize, usize, U), Error>::Err(error.into());
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

        Some(Ok((current_row, current_col, value)))
    }
}
