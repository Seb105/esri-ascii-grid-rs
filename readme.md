
# esri-ascii-grid-rs
### Rust library to read ESRI Ascii grid .asc files

Example ASCII Grid:
```
ncols         4
nrows         6
xllcorner     0.0
yllcorner     0.0
cellsize      50.0
NODATA_value  -9999
-9999 -9999 5 2
-9999 20 100 36
3 8 35 10
32 42 50 6
88 75 27 9
13 5 1 -9999
```

This library uses buffer readers to negate the need to load the entire ASCII grid into memory at once. The header wil be loaded and will allow you to check the properties of the header.

Example usage:

```rust
use esri_ascii_grid_rs::ascii_file::EsriASCIIReader;
let file = std::fs::File::open("test_data/test.asc").unwrap();
let mut grid = EsriASCIIReader::from_file(file).unwrap();
// Spot check a few values
assert_eq!(grid.get(390000.0, 344000.0).unwrap(), 141.2700042724609375);
assert_eq!(grid.get(390003.0, 344003.0).unwrap(), 135.44000244140625);

// Interpolate between cells
let val = grid.get_interpolate(grid.min_x() + grid.cell_size()/2., grid.min_y() + grid.cell_size()/2.).unwrap();

// Iterate over every cell
let grid_size = grid.num_rows() * grid.num_cols();
let iter = grid.into_iter();
let mut num_elements = 0;
for (row, col, value) in iter {
    num_elements += 1;
    if row == 3 && col == 3 {
        assert_eq!(value, 135.44000244140625);
    }
    if row == 0 && col == 0 {
        assert_eq!(value, 141.2700042724609375);
    }
}
assert_eq!(grid_size, num_elements);
```
