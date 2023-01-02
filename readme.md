
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

This library uses buffers to negate the need to load the entire ASCII grid into memory at once. The header will be loaded and will allow you to check the properties of the header. You can then either get specific values by index, coordinate or iterate over all points.

Example usage:

```rust
use esri_ascii_grid_rs::ascii_file::EsriASCIIReader;
let file = std::fs::File::open("test_data/test.asc").unwrap();
let mut grid = EsriASCIIReader::from_file(file).unwrap();
// Indexing the file is optional, but is recommended if you are going to be repeatedly calling any `get` function
// This will build the index and cache the file positions of each line, it will take a while for large files but will drastically increase subsequent get calls
grid.build_index().unwrap();
// Spot check a few values
assert_eq!(
    grid.get_index(994, 7).unwrap(),
    grid.header.no_data_value().unwrap()
);
assert_eq!(grid.get(390_000.0, 344_000.0).unwrap(), 141.270_004_272_460_937_5);
assert_eq!(grid.get(390_003.0, 344_003.0).unwrap(), 135.440_002_441_406_25);
assert_eq!(grid.get_index(3, 3).unwrap(), 135.440_002_441_406_25);
assert_eq!(grid.get_index(0, 0).unwrap(), 141.270_004_272_460_937_5);

// Interpolate between cells
let val = grid.get_interpolate(grid.header.min_x() + grid.header.cell_size()/4).unwrap();

// Iterate over every cell
let header = grid.header;
let grid_size = grid.header.num_rows() * grid.header.num_cols();
let iter = grid.into_iter();
let mut num_elements = 0;
for (row, col, value) in iter {
    num_elements += 1;
    if row == 3 && col == 3 {
        let (x, y) = header.index_pos(col, row).unwrap();
        assert_eq!(x, 390003.0);
        assert_eq!(y, 344003.0);
        assert_eq!(value, 135.44000244140625);
    }
    if row == 0 && col == 0 {
        let (x, y) = header.index_pos(col, row).unwrap();
        assert_eq!(x, 390000.0);
        assert_eq!(y, 344000.0);
        assert_eq!(value, 141.2700042724609375);
    }
}
assert_eq!(grid_size, num_elements);
```
