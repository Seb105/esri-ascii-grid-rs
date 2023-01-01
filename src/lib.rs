mod header;
pub mod ascii_file;
use std::io::{Read, Seek, BufReader, BufRead};

use header::EsriASCIIRasterHeader;


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
        assert_eq!(header.ncols as i32, 2000);
        assert_eq!(header.nrows as i32, 2000);
        assert_eq!(header.xll as i32, 390000);
        assert_eq!(header.yll as i32, 344000);
        assert_eq!(header.cornertype, crate::header::CornerType::Corner);
        assert_eq!(header.cellsize as i32, 1);
        assert_eq!(header.nodata_value, Some(-3.4028234663852885981e+38));
    }

    #[test]
    fn test_get_index() {
        let file = std::fs::File::open("test_data/test.asc").unwrap();
        let mut grid = crate::ascii_file::EsriASCIIReader::from_file(file).unwrap();
        // Spot check a few values
        assert_eq!(grid.get_index(1994, 7).unwrap(),  grid.no_data_value().unwrap());
        assert_eq!(grid.get_index(3, 3).unwrap(),  135.44000244140625);
        assert_eq!(grid.get_index(0, 0).unwrap(), 141.2700042724609375);

        // Check the bounds
        assert!(grid.get_index(0, 0).is_ok());
        assert!(grid.get_index(0, grid.num_cols()-1).is_ok());
        assert!(grid.get_index(grid.num_rows()-1, 0).is_ok());
        assert!(grid.get_index(grid.num_rows()-1, grid.num_cols()-1).is_ok());
        assert!(grid.get_index(0, grid.num_cols()).is_err());
        assert!(grid.get_index(grid.num_rows(), 0).is_err());
        assert!(grid.get_index(grid.num_rows(), grid.num_cols()).is_err());
    }

    #[test]
    fn test_get() {
        let file = std::fs::File::open("test_data/test.asc").unwrap();
        let mut grid = crate::ascii_file::EsriASCIIReader::from_file(file).unwrap();

        // Spot check a few values
        assert_eq!(grid.get(390000.0, 344000.0).unwrap(), 141.2700042724609375);
        assert_eq!(grid.get(390003.0, 344003.0).unwrap(), 135.44000244140625);


        // Check the bounds
        let min_x = grid.min_x();
        let min_y = grid.min_y();
        let max_x = grid.max_x();
        let max_y = grid.max_y();
        let cell_size = grid.cell_size();
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
        let grid_size = grid.num_rows() * grid.num_cols();
        let iter = grid.into_iter();
        let mut num_elements = 0;
        for (row, col, value) in iter {
            num_elements += 1;
            if row == 3 && col == 3 {
                // let (x, y) = grid.index_pos(col, row).unwrap();
                // assert_eq!(x, 390003.0);
                // assert_eq!(y, 344003.0);
                assert_eq!(value, 135.44000244140625);
            }
            if row == 0 && col == 0 {
                // let (x, y) = grid.index_pos(col, row).unwrap();
                // assert_eq!(x, 390000.0);
                // assert_eq!(y, 344000.0);
                assert_eq!(value, 141.2700042724609375);
            }
        }
        assert_eq!(grid_size, num_elements);
    }

    #[test]
    fn test_index_of() {
        let file = std::fs::File::open("test_data/test.asc").unwrap();
        let mut grid = crate::ascii_file::EsriASCIIReader::from_file(file).unwrap();
        assert_eq!(grid.index_of(grid.min_x(), grid.min_y()).unwrap(), (0, 0));
        assert_eq!(grid.index_of(grid.max_x(), grid.max_y()).unwrap(), (grid.num_cols()-1, grid.num_rows()-1));
        assert_eq!(grid.index_of(grid.min_x(), grid.max_y()).unwrap(), (0, grid.num_rows()-1));
        assert_eq!(grid.index_of(grid.max_x(), grid.min_y()).unwrap(), (grid.num_cols()-1, 0)); 

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
        let val1 = grid.get_interpolate(grid.min_x() + grid.cell_size()/2., grid.min_y() + grid.cell_size()/2.).unwrap();
        assert_eq!(val1, expected1);

        let expected2 = ll*0.5625 + lr*0.1875 + ul*0.1875 + ur*0.0625;
        let val2 = grid.get_interpolate(grid.min_x() + grid.cell_size()/4., grid.min_y() + grid.cell_size()/4.).unwrap();
        assert_eq!(val2, expected2);

        // Bounds check
        let min_x = grid.min_x();
        let min_y = grid.min_y();
        let max_x = grid.max_x();
        let max_y = grid.max_y();
        let cell_size = grid.cell_size();
        assert_eq!(grid.get_interpolate(min_x, min_y).unwrap(), grid.get_index(0, 0).unwrap());
        assert_eq!(grid.get_interpolate(max_x, max_y).unwrap(), grid.get_index(grid.num_rows()-1, grid.num_cols()-1).unwrap());
        assert_eq!(grid.get_interpolate(min_x, max_y).unwrap(), grid.get_index(0, grid.num_cols()-1).unwrap());
        assert_eq!(grid.get_interpolate(max_x, min_y).unwrap(), grid.get_index(grid.num_rows()-1, 0).unwrap());
        assert!(grid.get_interpolate(min_x - cell_size, min_y).is_none());
        assert!(grid.get_interpolate(min_x, min_y - cell_size).is_none());
        assert!(grid.get_interpolate(max_x + cell_size, max_y).is_none());
        assert!(grid.get_interpolate(max_x, max_y + cell_size).is_none());
    }
}
