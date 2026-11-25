//! Fuzz style tests for the CSV reader: it must never panic and must keep to its limits.

use kh_formats::{CsvLimits, read_csv};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn arbitrary_bytes_never_panic_and_respect_limits(
        data in proptest::collection::vec(any::<u8>(), 0..8192),
        delimiter in prop::sample::select(vec![b',', b';', b'\t']),
    ) {
        let limits = CsvLimits {
            max_rows: Some(50),
            max_cell_bytes: 64,
            max_columns: 16,
            max_record_bytes: 512,
            max_total_bytes: 4096,
        };
        if let Ok((table, _)) = read_csv(&data[..], delimiter, &limits) {
            prop_assert!(table.column_count() <= limits.max_columns);
            prop_assert!(table.row_count() <= 50);
            for r in 0..table.row_count() {
                for c in 0..table.column_count() {
                    let cell = table.cell(r, c).unwrap();
                    // Bad bytes become 3 byte replacement characters, so allow for that growth.
                    prop_assert!(cell.len() <= limits.max_cell_bytes * 3);
                }
            }
        }
    }

    #[test]
    fn written_tables_read_back_the_same(
        rows in proptest::collection::vec(
            proptest::collection::vec("[a-z0-9 ,\"\n;]{1,12}", 3),
            1..30,
        ),
    ) {
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer.write_record(["one", "two", "three"]).unwrap();
        for row in &rows {
            writer.write_record(row).unwrap();
        }
        let bytes = writer.into_inner().unwrap();
        let (table, report) = read_csv(&bytes[..], b',', &CsvLimits::default()).unwrap();
        prop_assert_eq!(table.row_count(), rows.len());
        prop_assert_eq!(report.short_rows, 0);
        for (r, row) in rows.iter().enumerate() {
            for (c, expected) in row.iter().enumerate() {
                prop_assert_eq!(table.cell(r, c), Some(expected.as_str()));
            }
        }
    }
}
