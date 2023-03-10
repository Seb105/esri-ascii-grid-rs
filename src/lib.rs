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
//! use esri_ascii_grid::ascii_file::EsriASCIIReader;
//! let file = std::fs::File::open("test_data/test.asc").unwrap();
//! let mut grid = EsriASCIIReader::from_file(file).unwrap();
//! // Indexing the file is optional, but is recommended if you are going to be repeatedly calling any `get` function
//! // This will build the index and cache the file positions of each line, it will take a while for large files 
//! // but will drastically increase the speed subsequent `get` calls.
//! grid.build_index().unwrap();
//! // Spot check a few values
//! assert_eq!(
//!     grid.get_index(994, 7).unwrap(),
//!     grid.header.no_data_value().unwrap()
//! );
//! assert_eq!(grid.get(390_000.0, 344_000.0).unwrap(), 141.270_004_272_460_937_5);
//! assert_eq!(grid.get(390_003.0, 344_003.0).unwrap(), 135.440_002_441_406_25);
//! assert_eq!(grid.get_index(3, 3).unwrap(), 135.440_002_441_406_25);
//! assert_eq!(grid.get_index(0, 0).unwrap(), 141.270_004_272_460_937_5);
//!
//! // Interpolate between cells
//! let val = grid.get_interpolate(grid.header.min_x() + grid.header.cell_size()/4., grid.header.min_y() + grid.header.cell_size()/4.).unwrap();
//!
//! // Iterate over every cell
//! let header = grid.header;
//! let grid_size = grid.header.num_rows() * grid.header.num_cols();
//! let iter = grid.into_iter();
//! let mut num_elements = 0;
//! for (row, col, value) in iter {
//!     num_elements += 1;
//!     if row == 3 && col == 3 {
//!         let (x, y) = header.index_pos(col, row).unwrap();
//!         assert_eq!(x, 390003.0);
//!         assert_eq!(y, 344003.0);
//!         assert_eq!(value, 135.44000244140625);
//!     }
//!     if row == 0 && col == 0 {
//!         let (x, y) = header.index_pos(col, row).unwrap();
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
pub mod header;
#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use crate::header::EsriASCIIRasterHeader;

    #[test]
    fn test_header() {
        let file = std::fs::File::open("test_data/test.asc").unwrap();
        let mut reader = BufReader::new(file);
        let header = EsriASCIIRasterHeader::from_reader(&mut reader);
        assert!(header.is_ok());
        let header = header.unwrap();
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
        let file = std::fs::File::open("test_data/test.asc").unwrap();
        let mut grid = crate::ascii_file::EsriASCIIReader::from_file(file).unwrap();
        // Spot check a few values
        assert_eq!(
            grid.get_index(994, 7).unwrap(),
            grid.header.no_data_value().unwrap()
        );
        assert_eq!(grid.get_index(3, 3).unwrap(), 135.440_002_441_406_25);
        assert_eq!(grid.get_index(0, 0).unwrap(), 141.270_004_272_460_937_5);

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
        let file = std::fs::File::open("test_data/test.asc").unwrap();
        let mut grid = crate::ascii_file::EsriASCIIReader::from_file(file).unwrap();

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
        let file = std::fs::File::open("test_data/test.asc").unwrap();
        let grid = crate::ascii_file::EsriASCIIReader::from_file(file).unwrap();
        let header = grid.header;
        let grid_size = grid.header.num_rows() * grid.header.num_cols();
        let iter = grid.into_iter();
        let mut num_elements = 0;
        for (row, col, value) in iter {
            num_elements += 1;
            if row == 3 && col == 3 {
                let (x, y) = header.index_pos(col, row).unwrap();
                assert_eq!(x, 390_003.0);
                assert_eq!(y, 344_003.0);
                assert_eq!(value, 135.440_002_441_406_25);
            }
            if row == 0 && col == 0 {
                let (x, y) = header.index_pos(col, row).unwrap();
                assert_eq!(x, 390_000.0);
                assert_eq!(y, 344_000.0);
                assert_eq!(value, 141.270_004_272_460_937_5);
            }
        }
        assert_eq!(grid_size, num_elements);
    }

    #[test]
    fn test_index_of() {
        let file = std::fs::File::open("test_data/test.asc").unwrap();
        let mut grid = crate::ascii_file::EsriASCIIReader::from_file(file).unwrap();
        assert_eq!(
            grid.header
                .index_of(grid.header.min_x(), grid.header.min_y())
                .unwrap(),
            (0, 0)
        );
        assert_eq!(
            grid.header
                .index_of(grid.header.max_x(), grid.header.max_y())
                .unwrap(),
            (grid.header.num_cols() - 1, grid.header.num_rows() - 1)
        );
        assert_eq!(
            grid.header
                .index_of(grid.header.min_x(), grid.header.max_y())
                .unwrap(),
            (0, grid.header.num_rows() - 1)
        );
        assert_eq!(
            grid.header
                .index_of(grid.header.max_x(), grid.header.min_y())
                .unwrap(),
            (grid.header.num_cols() - 1, 0)
        );
    }

    #[test]
    fn test_get_interp() {
        let file = std::fs::File::open("test_data/test.asc").unwrap();
        let mut grid = crate::ascii_file::EsriASCIIReader::from_file(file).unwrap();
        let ll = grid.get_index(0, 0).unwrap();
        let lr = grid.get_index(0, 1).unwrap();
        let ul = grid.get_index(1, 0).unwrap();
        let ur = grid.get_index(1, 1).unwrap();

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

        // Bounds check
        let min_x = grid.header.min_x();
        let min_y = grid.header.min_y();
        let max_x = grid.header.max_x();
        let max_y = grid.header.max_y();
        let cell_size = grid.header.cell_size();
        assert_eq!(
            grid.get_interpolate(min_x, min_y).unwrap(),
            grid.get_index(0, 0).unwrap()
        );
        assert_eq!(
            grid.get_interpolate(max_x, max_y).unwrap(),
            grid.get_index(grid.header.num_rows() - 1, grid.header.num_cols() - 1)
                .unwrap()
        );
        assert_eq!(
            grid.get_interpolate(min_x, max_y).unwrap(),
            grid.get_index(0, grid.header.num_cols() - 1).unwrap()
        );
        assert_eq!(
            grid.get_interpolate(max_x, min_y).unwrap(),
            grid.get_index(grid.header.num_rows() - 1, 0).unwrap()
        );
        assert!(grid.get_interpolate(min_x - cell_size, min_y).is_none());
        assert!(grid.get_interpolate(min_x, min_y - cell_size).is_none());
        assert!(grid.get_interpolate(max_x + cell_size, max_y).is_none());
        assert!(grid.get_interpolate(max_x, max_y + cell_size).is_none());
    }

    #[test]
    fn test_many_gets() {
        let file = std::fs::File::open("test_data/test.asc").unwrap();
        let mut grid = crate::ascii_file::EsriASCIIReader::from_file(file).unwrap();
        for x in 0..grid.header.ncols {
            for y in 0..grid.header.nrows {
                let x_pos = x as f64 * grid.header.cell_size() + grid.header.min_x();
                let y_pos = y as f64 * grid.header.cell_size() + grid.header.min_y();
                let val = grid.get(x_pos, y_pos).unwrap();
                let val2 = grid.get_index(y, x).unwrap();
                assert_eq!(val, val2);
            }
        }
    }

    #[test]
    fn test_many_gets_indexed() {
        let file = std::fs::File::open("test_data/test.asc").unwrap();
        let mut grid = crate::ascii_file::EsriASCIIReader::from_file(file).unwrap();
        grid.build_index().unwrap();
        for x in 0..grid.header.ncols {
            for y in 0..grid.header.nrows {
                let x_pos = x as f64 * grid.header.cell_size() + grid.header.min_x();
                let y_pos = y as f64 * grid.header.cell_size() + grid.header.min_y();
                let val = grid.get(x_pos, y_pos).unwrap();
                let val2 = grid.get_index(y, x).unwrap();
                assert_eq!(val, val2);
            }
        }
    }
}
