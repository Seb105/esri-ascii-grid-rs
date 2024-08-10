//! # esri-ascii-grid-rs
//! ### Rust library to read ESRI Ascii grid .asc files
//!
//! Example ASCII Grid:
//! ```text
//! ncols         4
//! nrows         6
//! xllcorner     0.0
//! yllcorner     0.0
//! cellsize      50.0
//! NODATA_value  -9999
//! -9999 -9999 5 2
//! -9999 20 100 36
//! 3 8 35 10
//! 32 42 50 6
//! 88 75 27 9
//! 13 5 1 -9999
//! ```
//!
//! This library uses buffers to negate the need to load the entire ASCII grid into memory at once. The header will be loaded and will allow you to check the properties of the header. You can then either get specific values by index, coordinate or iterate over all points.
//!
//! ## Usage:
//!
//! ```rust
//! use std::fs::File;
//! use esri_ascii_grid::ascii_file::EsriASCIIReader;
//! let file = File::open("test_data/test.asc").unwrap();
//! let mut grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();
//! // Spot check a few values
//! assert_eq!(
//!     grid.get_index(5, 7).unwrap(),
//!     grid.header.no_data_value().unwrap()
//! );
//! assert_eq!(grid.get(390_000.0, 344_000.0).unwrap(), 141.270_004_272_460_937_5);
//! assert_eq!(grid.get(390_003.0, 344_003.0).unwrap(), 135.440_002_441_406_25);
//! assert_eq!(grid.get_index(996, 3).unwrap(), 135.440_002_441_406_25);
//! assert_eq!(grid.get_index(999, 0).unwrap(), 141.270_004_272_460_937_5);
//!
//! // Interpolate between cells
//! let val = grid.get_interpolate(grid.header.min_x() + grid.header.cell_size()/4., grid.header.min_y() + grid.header.cell_size()/4.).unwrap();
//!
//! // Iterate over every cell
//! let header = grid.header;
//! let grid_size = grid.header.num_rows() * grid.header.num_cols();
//! let iter = grid.into_iter();
//! let mut num_elements = 0;
//! for cell in iter {
//!     let Ok((row, col, value)) = cell else {
//!         panic!("your error handler")
//!     };
//!     num_elements += 1;
//!     if row == 996 && col == 3 {
//!         let (x, y) = header.index_pos(row, col).unwrap();
//!         assert_eq!(x, 390003.0);
//!         assert_eq!(y, 344003.0);
//!         assert_eq!(value, 135.44000244140625);
//!     }
//!     if row == header.nrows-1 && col == 0 {
//!         let (x, y) = header.index_pos(row, col).unwrap();
//!         assert_eq!(x, 390000.0);
//!         assert_eq!(y, 344000.0);
//!         assert_eq!(value, 141.2700042724609375);
//!     }
//! }
//! assert_eq!(grid_size, num_elements);
//! ```

#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::excessive_precision,
    clippy::module_name_repetitions,
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::cast_precision_loss,
    clippy::float_cmp
)]
pub mod ascii_file;
pub mod error;
pub mod header;

pub use error::Error;

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{BufReader, Read, Seek},
    };

    use crate::{
        ascii_file::EsriASCIIReader,
        error,
        header::{EsriASCIIRasterHeader, Numerical},
    };

    #[test]
    fn test_header() {
        let file = File::open("test_data/test.asc").unwrap();
        let mut reader = BufReader::new(file);
        let header = EsriASCIIRasterHeader::from_reader(&mut reader);
        assert!(header.is_ok());
        let header: EsriASCIIRasterHeader<f64, f64> = header.unwrap();
        assert_eq!(header.ncols, 2000);
        assert_eq!(header.nrows, 1000);
        assert_eq!(header.xll as i32, 390_000);
        assert_eq!(header.yll as i32, 344_000);
        assert_eq!(header.cornertype, crate::header::CornerType::Corner);
        assert_eq!(header.cellsize as i32, 1);
        assert_eq!(header.nodata_value, Some(-3.402_823_466_385_288_598_1e+38));
    }

    #[test]
    fn test_get_index() {
        let file = File::open("test_data/test.asc").unwrap();
        let mut grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();
        // Spot check a few values
        assert_eq!(
            grid.get_index(6, 7).unwrap(),
            grid.header.no_data_value().unwrap()
        );
        assert_eq!(grid.get_index(996, 3).unwrap(), 135.440_002_441_406_25);
        assert_eq!(grid.get_index(999, 0).unwrap(), 141.270_004_272_460_937_5);

        // Check the bounds
        assert!(grid.get_index(0, 0).is_ok());
        assert!(grid.get_index(0, grid.header.num_cols() - 1).is_ok());
        assert!(grid.get_index(grid.header.num_rows() - 1, 0).is_ok());
        assert!(grid
            .get_index(grid.header.num_rows() - 1, grid.header.num_cols() - 1)
            .is_ok());
        assert!(grid.get_index(0, grid.header.num_cols()).is_err());
        assert!(grid.get_index(grid.header.num_rows(), 0).is_err());
        assert!(grid
            .get_index(grid.header.num_rows(), grid.header.num_cols())
            .is_err());
    }

    #[test]
    fn test_get() {
        let file = File::open("test_data/test.asc").unwrap();
        let mut grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();

        // Spot check a few values
        assert_eq!(
            grid.get(390_000.0, 344_000.0).unwrap(),
            141.270_004_272_460_937_5
        );
        assert_eq!(
            grid.get(390_003.0, 344_003.0).unwrap(),
            135.440_002_441_406_25
        );

        // Check the bounds
        let min_x = grid.header.min_x();
        let min_y = grid.header.min_y();
        let max_x = grid.header.max_x();
        let max_y = grid.header.max_y();
        let cell_size = grid.header.cell_size();
        assert!(grid.get(min_x, min_y).is_some());
        assert!(grid.get(max_x, max_y).is_some());
        assert!(grid.get(min_x, max_y).is_some());
        assert!(grid.get(max_x, min_y).is_some());
        assert!(grid.get(min_x - cell_size, min_y).is_none());
        assert!(grid.get(min_x, min_y - cell_size).is_none());
        assert!(grid.get(max_x + cell_size, max_y).is_none());
        assert!(grid.get(max_x, max_y + cell_size).is_none());
    }

    #[test]
    fn test_iter() {
        let file = File::open("test_data/test.asc").unwrap();
        let grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();
        let header = grid.header;
        let grid_size = grid.header.num_rows() * grid.header.num_cols();
        let iter = grid.into_iter();
        let mut num_elements = 0;
        for cell in iter {
            let (row, col, value) = cell.unwrap();
            num_elements += 1;
            if row == 996 && col == 3 {
                let (x, y) = header.index_pos(row, col).unwrap();
                assert_eq!(x, 390_003.0);
                assert_eq!(y, 344_003.0);
                assert_eq!(value, 135.440_002_441_406_25);
            }
            if row == header.nrows - 1 && col == 0 {
                let (x, y) = header.index_pos(row, col).unwrap();
                assert_eq!(x, 390_000.0);
                assert_eq!(y, 344_000.0);
                assert_eq!(value, 141.270_004_272_460_937_5);
            }
        }
        assert_eq!(grid_size, num_elements);
    }
    #[test]
    fn test_index_pos() {
        let file = File::open("test_data/test.asc").unwrap();
        let grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();
        let cell_size = grid.header.cell_size();
        // - cell_size because max_x/y is the top right corner of the cell, but the index_pos is the bottom left corner
        let max_index_x = grid.header.max_x() - cell_size;
        let max_index_y = grid.header.max_y() - cell_size;
        let min_index_x = grid.header.min_x();
        let min_index_y = grid.header.min_y();
        assert_eq!(
            grid.header.index_pos(0, 0).unwrap(),
            (min_index_x, max_index_y)
        );
        assert_eq!(
            grid.header
                .index_pos(grid.header.num_rows() - 1, grid.header.num_cols() - 1)
                .unwrap(),
            (max_index_x, min_index_y)
        );
        assert_eq!(
            grid.header
                .index_pos(grid.header.num_rows() - 1, 0)
                .unwrap(),
            (min_index_x, min_index_y)
        );
        assert_eq!(
            grid.header
                .index_pos(0, grid.header.num_cols() - 1)
                .unwrap(),
            (max_index_x, max_index_y)
        );
    }
    #[test]
    fn test_index_of() {
        let file = File::open("test_data/test.asc").unwrap();
        let grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();
        assert_eq!(
            grid.header
                .index_of(grid.header.min_x(), grid.header.min_y())
                .unwrap(),
            (grid.header.num_rows() - 1, 0)
        );
        assert_eq!(
            grid.header
                .index_of(grid.header.max_x(), grid.header.max_y())
                .unwrap(),
            (0, grid.header.num_cols() - 1)
        );
        assert_eq!(
            grid.header
                .index_of(grid.header.min_x(), grid.header.max_y())
                .unwrap(),
            (0, 0)
        );
        assert_eq!(
            grid.header
                .index_of(grid.header.max_x(), grid.header.min_y())
                .unwrap(),
            (grid.header.num_rows() - 1, grid.header.num_cols() - 1)
        );
    }

    #[test]
    fn test_get_interp() {
        let file = File::open("test_data/test.asc").unwrap();
        let mut grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();
        let ll = grid.get_index(999, 0).unwrap();
        let lr = grid.get_index(999, 1).unwrap();
        let ul = grid.get_index(998, 0).unwrap();
        let ur = grid.get_index(998, 1).unwrap();

        // Spot check a few values
        let expected1 = (ll + lr + ul + ur) / 4.;
        let val1 = grid
            .get_interpolate(
                grid.header.min_x() + grid.header.cell_size() / 2.,
                grid.header.min_y() + grid.header.cell_size() / 2.,
            )
            .unwrap();
        assert_eq!(val1, expected1);

        let expected2 = ll * 0.5625 + lr * 0.1875 + ul * 0.1875 + ur * 0.0625;
        let val2 = grid
            .get_interpolate(
                grid.header.min_x() + grid.header.cell_size() / 4.,
                grid.header.min_y() + grid.header.cell_size() / 4.,
            )
            .unwrap();
        assert_eq!(val2, expected2);

        // At max_x and max_y there are fewer cells to interpolate with, so the interpolated value will be the same as the value as the lower left cell
        assert_eq!(
            grid.get_interpolate(
                grid.header.max_x() - grid.header.cell_size() / 2.,
                grid.header.max_y() - grid.header.cell_size() / 2.
            )
            .unwrap(),
            grid.get_index(grid.header.num_rows() - 1, grid.header.num_cols() - 1)
                .unwrap()
        );

        // Bounds check
        let min_x = grid.header.min_x();
        let min_y = grid.header.min_y();
        let max_x = grid.header.max_x();
        let max_y = grid.header.max_y();
        let cell_size = grid.header.cell_size();
        assert_eq!(
            grid.get_interpolate(min_x, min_y).unwrap(),
            grid.get_index(grid.header.num_rows() - 1, 0).unwrap()
        );
        assert_eq!(
            grid.get_interpolate(max_x, max_y).unwrap(),
            grid.get_index(0, grid.header.num_cols() - 1).unwrap()
        );
        assert_eq!(
            grid.get_interpolate(min_x, max_y).unwrap(),
            grid.get_index(0, 0).unwrap()
        );
        assert_eq!(
            grid.get_interpolate(max_x, min_y).unwrap(),
            grid.get_index(grid.header.num_rows() - 1, grid.header.num_cols() - 1)
                .unwrap()
        );
        assert!(grid.get_interpolate(min_x - cell_size, min_y).is_none());
        assert!(grid.get_interpolate(min_x, min_y - cell_size).is_none());
        assert!(grid.get_interpolate(max_x + cell_size, max_y).is_none());
        assert!(grid.get_interpolate(max_x, max_y + cell_size).is_none());
    }

    #[test]
    fn test_many_gets() {
        let file = File::open("test_data/test.asc").unwrap();
        let mut grid: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(file).unwrap();
        let header = grid.header;
        for row in 0..grid.header.nrows {
            for col in 0..grid.header.ncols {
                let x_pos = grid.header.min_x() + col as f64 * grid.header.cell_size();
                let y_pos = grid.header.max_y()
                    - row as f64 * grid.header.cell_size()
                    - grid.header.cell_size();
                let index_of = grid.header.index_of(x_pos, y_pos).unwrap();
                assert_eq!(index_of, (row, col));
                let val = grid.get(x_pos, y_pos).unwrap();
                let val2 = grid.get_index(row, col).unwrap();
                assert_eq!(val, val2);
                if row == 996 && col == 3 {
                    let (x, y) = header.index_pos(row, col).unwrap();
                    assert_eq!(x, 390_003.0);
                    assert_eq!(y, 344_003.0);
                    assert_eq!(val, 135.440_002_441_406_25);
                }
                if row == header.nrows - 1 && col == 0 {
                    let (x, y) = header.index_pos(row, col).unwrap();
                    assert_eq!(x, 390_000.0);
                    assert_eq!(y, 344_000.0);
                    assert_eq!(val, 141.270_004_272_460_937_5);
                }
            }
        }
    }
    #[test]
    fn test_corner_types() {
        let xll = 0.; // From the test data files.
        let yll = 0.;

        let type_corner = File::open("test_data/test_llcorner.asc").unwrap();
        let type_center = File::open("test_data/test_llcenter.asc").unwrap();
        let grid_corner: EsriASCIIReader<File, f64, f64> =
            EsriASCIIReader::from_file(type_corner).unwrap();
        let grid_center: EsriASCIIReader<File, f64, f64> =
            EsriASCIIReader::from_file(type_center).unwrap();

        let header_center = grid_center.header;
        let header_corner = grid_corner.header;

        // Assert that everything is the same except for the corner type
        assert_eq!(header_center.ncols, header_corner.ncols);
        assert_eq!(header_center.nrows, header_corner.nrows);
        assert_eq!(header_center.cellsize, header_corner.cellsize);
        assert_eq!(header_center.nodata_value, header_corner.nodata_value);
        // Collect both iterators and confirm that they are the same
        let iter_center = grid_center.into_iter();
        let iter_corner = grid_corner.into_iter();
        for (cell_center, cell_corner) in iter_center.zip(iter_corner) {
            let (row_center, col_center, value_center) = cell_center.unwrap();
            let (row_corner, col_corner, value_corner) = cell_corner.unwrap();
            assert_eq!(row_center, row_corner);
            assert_eq!(col_center, col_corner);
            assert_eq!(value_center, value_corner);
        }
        // Check the bounds. If the corner type is corner, the min_x and min_y will be the same as the xll and yll
        // Therefore, the min_x and min_y will be half a cell size smaller than the yllcentre and xllcentre
        assert_eq!(
            header_center.min_x(),
            header_corner.min_x() - header_center.cell_size() / 2.0
        );
        assert_eq!(
            header_center.min_y(),
            header_corner.min_y() - header_center.cell_size() / 2.0
        );

        assert_eq!(header_center.min_x(), xll - header_center.cell_size() / 2.0);
        assert_eq!(header_center.min_y(), yll - header_center.cell_size() / 2.0);

        // However, the range covered by the grid should be the same
        let range_x = 200.;
        let range_y = 300.;
        assert_eq!(header_center.max_x() - header_center.min_x(), range_x);
        assert_eq!(header_center.max_y() - header_center.min_y(), range_y);
        assert_eq!(header_corner.max_x() - header_corner.min_x(), range_x);
        assert_eq!(header_corner.max_y() - header_corner.min_y(), range_y);

        let type_corner = File::open("test_data/test_llcorner.asc").unwrap();
        let type_center = File::open("test_data/test_llcenter.asc").unwrap();
        let mut grid_center: EsriASCIIReader<File, f64, f64> =
            EsriASCIIReader::from_file(type_center).unwrap();
        let mut grid_corner: EsriASCIIReader<File, f64, f64> =
            EsriASCIIReader::from_file(type_corner).unwrap();
        // We can still get the extremes ok
        grid_center
            .get(header_center.min_x(), header_center.min_y())
            .unwrap();
        grid_center
            .get(header_center.max_x(), header_center.max_y())
            .unwrap();
        grid_center
            .get(header_center.min_x(), header_center.max_y())
            .unwrap();
        grid_center
            .get(header_center.max_x(), header_center.min_y())
            .unwrap();

        grid_corner
            .get(header_corner.min_x(), header_corner.min_y())
            .unwrap();
        grid_corner
            .get(header_corner.max_x(), header_corner.max_y())
            .unwrap();
        grid_corner
            .get(header_corner.min_x(), header_corner.max_y())
            .unwrap();
        grid_corner
            .get(header_corner.max_x(), header_corner.min_y())
            .unwrap();
    }
    #[test]
    fn test_generics() {
        // std::env::set_var("RUST_BACKTRACE", "full");
        struct MultipleGrids<R, A, B, C, D, E, F>
        where
            R: Read + Seek,
            A: Numerical,
            B: Numerical,
            C: Numerical,
            D: Numerical,
            E: Numerical,
            F: Numerical,
        {
            grid_a: EsriASCIIReader<R, A, A>,
            grid_b: EsriASCIIReader<R, B, B>,
            grid_c: EsriASCIIReader<R, C, C>,
            grid_d: EsriASCIIReader<R, D, D>,
            grid_e: EsriASCIIReader<R, E, E>,
            grid_f: EsriASCIIReader<R, F, F>,
        }
        impl<R, A, B, C, D, E, F> MultipleGrids<R, A, B, C, D, E, F>
        where
            R: Read + Seek,
            A: Numerical,
            error::Error: From<<A as Numerical>::Err>,
            B: Numerical,
            error::Error: From<<B as Numerical>::Err>,
            C: Numerical,
            error::Error: From<<C as Numerical>::Err>,
            D: Numerical,
            error::Error: From<<D as Numerical>::Err>,
            E: Numerical,
            error::Error: From<<E as Numerical>::Err>,
            F: Numerical,
            error::Error: From<<F as Numerical>::Err>,
        {
            fn new(
                grid_a: EsriASCIIReader<R, A, A>,
                grid_b: EsriASCIIReader<R, B, B>,
                grid_c: EsriASCIIReader<R, C, C>,
                grid_d: EsriASCIIReader<R, D, D>,
                grid_e: EsriASCIIReader<R, E, E>,
                grid_f: EsriASCIIReader<R, F, F>,
            ) -> Self {
                Self {
                    grid_a,
                    grid_b,
                    grid_c,
                    grid_d,
                    grid_e,
                    grid_f,
                }
            }
            fn get_all(&mut self, x: f64, y: f64) -> (A, B, C, D, E, F) {
                let a = self
                    .grid_a
                    .get(A::from(x).unwrap(), A::from(y).unwrap())
                    .unwrap();
                let b = self
                    .grid_b
                    .get(B::from(x).unwrap(), B::from(y).unwrap())
                    .unwrap();
                let c = self
                    .grid_c
                    .get(C::from(x).unwrap(), C::from(y).unwrap())
                    .unwrap();
                let d = self
                    .grid_d
                    .get(D::from(x).unwrap(), D::from(y).unwrap())
                    .unwrap();
                let e = self
                    .grid_e
                    .get(E::from(x).unwrap(), E::from(y).unwrap())
                    .unwrap();
                let f = self
                    .grid_f
                    .get(F::from(x).unwrap(), F::from(y).unwrap())
                    .unwrap();
                return (a, b, c, d, e, f);
            }
            fn compare_to(&mut self, x: f64, y: f64, value: f64) {
                let (a, b, c, d, e, f) = self.get_all(x, y);
                assert_eq!(a, A::from(value).unwrap());
                assert_eq!(b, B::from(value).unwrap());
                assert_eq!(c, C::from(value).unwrap());
                assert_eq!(d, D::from(value).unwrap());
                assert_eq!(e, E::from(value).unwrap());
                assert_eq!(f, F::from(value).unwrap());
            }
        }
        let test_path = "test_data/test_ints.asc";
        let fa = File::open(test_path).unwrap();
        let fb = File::open(test_path).unwrap();
        let fc = File::open(test_path).unwrap();
        let fd = File::open(test_path).unwrap();
        let fe = File::open(test_path).unwrap();
        let ff = File::open(test_path).unwrap();
        let grid_i16: EsriASCIIReader<File, i16, i16> = EsriASCIIReader::from_file(fa).unwrap();
        let grid_i32: EsriASCIIReader<File, i32, i32> = EsriASCIIReader::from_file(fb).unwrap();
        let grid_i128: EsriASCIIReader<File, i128, i128> = EsriASCIIReader::from_file(fc).unwrap();
        let grid_i64: EsriASCIIReader<File, i64, i64> = EsriASCIIReader::from_file(fd).unwrap();
        let grid_f32: EsriASCIIReader<File, f32, f32> = EsriASCIIReader::from_file(fe).unwrap();
        let grid_f64: EsriASCIIReader<File, f64, f64> = EsriASCIIReader::from_file(ff).unwrap();
        let mut multiple_grids =
            MultipleGrids::new(grid_i16, grid_i32, grid_i64, grid_i128, grid_f32, grid_f64);
        // Check that we can get all the values
        multiple_grids.compare_to(100., 150., 35.);
    }

    #[cfg(feature = "ordered-float")]
    #[test]
    fn can_parse_into_notnan() {
        use ordered_float::NotNan;

        let file = File::open("test_data/test.asc").unwrap();
        let grid = EsriASCIIReader::<_, NotNan<f64>, NotNan<f64>>::from_file(file).unwrap();
        assert!(grid.into_iter().all(|cell| cell.is_ok()));
    }
}
