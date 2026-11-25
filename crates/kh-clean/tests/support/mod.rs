//! A seeded generator for the survey fixture. Every count the tests check is decided here.
#![allow(dead_code, missing_docs, clippy::unwrap_used)]

pub const HEADER: [&str; 5] = ["id", "date", "market", "product", "price"];
pub const MARKET_SPELLINGS: [&str; 7] = [
    "Kisenyi", "kisenyi", "KISENYI", "Kisenyi ", "Kiseny", "Kisenyii", "Kissenyi",
];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const OTHER_MARKETS: [&str; 2] = ["Owino", "Nakasero"];
const PRODUCTS: [&str; 5] = ["maize", "beans", "rice", "sugar", "millet"];

#[derive(Clone, Copy)]
pub struct Spec {
    pub seed: u64,
    pub week: u32,
    pub base_rows: usize,
    pub duplicates: usize,
    pub empty_prices: usize,
    pub unreadable_dates: usize,
    pub trailing_products: usize,
}

pub const WEEK3: Spec = Spec {
    seed: 3,
    week: 3,
    base_rows: 400,
    duplicates: 12,
    empty_prices: 9,
    unreadable_dates: 5,
    trailing_products: 11,
};
pub const WEEK4: Spec = Spec {
    seed: 4,
    week: 4,
    base_rows: 300,
    duplicates: 8,
    empty_prices: 6,
    unreadable_dates: 4,
    trailing_products: 7,
};

/// What the generator knows about the file it made.
pub struct Fixture {
    pub csv: String,
    pub rows_in: usize,
    pub rows_out: usize,
    pub duplicate_rows: usize,
    pub empty_prices: usize,
    pub unreadable_dates: usize,
    pub rows_needing_person: usize,
    /// Cells in the raw file with a space at the start or end.
    pub space_cells: usize,
    /// Cells in the raw file that are wrong in any way (spaces, date shape, price shape, empty price).
    pub wrong_cells: usize,
    pub market_spellings_seen: usize,
    /// The file after the saved steps, header not included.
    pub expected: Vec<Vec<String>>,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn date_text(format: usize, day: usize) -> String {
    let (m, y) = (11, 2026);
    match format % 4 {
        0 => format!("{day:02}/{m}/{y}"),
        1 => format!("{day} {} {y}", MONTHS[m - 1]),
        2 => format!("{y}-{m:02}-{day:02}"),
        _ => format!("{day} November {y}"),
    }
}

fn price_text(format: usize, price: usize) -> String {
    let commas = format!("{},{:03}", price / 1000, price % 1000);
    match format % 4 {
        0 => format!("UGX {commas}"),
        1 => price.to_string(),
        2 => format!("UGX{commas}"),
        _ => commas,
    }
}

struct Record {
    raw: Vec<String>,
    clean: Vec<String>,
    special: bool,
}

pub fn generate(spec: Spec) -> Fixture {
    let mut rng = Rng(spec.seed);
    let mut order: Vec<usize> = (0..spec.base_rows).collect();
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i + 1));
    }
    let take = |from: usize, n: usize| -> Vec<usize> { order[from..from + n].to_vec() };
    let empties = take(0, spec.empty_prices);
    let unreadable = take(spec.empty_prices, spec.unreadable_dates);
    let trailing = take(
        spec.empty_prices + spec.unreadable_dates,
        spec.trailing_products,
    );
    let first_ordinary = spec.empty_prices + spec.unreadable_dates + spec.trailing_products;

    let first_day = 7 * (spec.week as usize - 1) + 1;
    let mut kisenyi_seen = 0;
    let mut records: Vec<Record> = (0..spec.base_rows)
        .map(|i| {
            let day = first_day + rng.below(7);
            let kisenyi = i < 7 || rng.below(2) == 0;
            let market = if kisenyi {
                kisenyi_seen += 1;
                // The first seven Kisenyi rows use the seven spellings, so all seven always appear.
                let k = if kisenyi_seen <= 7 {
                    kisenyi_seen - 1
                } else if rng.below(3) == 0 {
                    rng.below(7)
                } else {
                    0
                };
                MARKET_SPELLINGS[k].to_string()
            } else {
                OTHER_MARKETS[rng.below(2)].to_string()
            };
            let product = PRODUCTS[rng.below(5)];
            let price = 1000 + 100 * rng.below(90);
            let id = format!("W{}-{:04}", spec.week, i + 1);
            let is = |set: &[usize]| set.contains(&i);
            let date = if is(&unreadable) {
                format!("{day}-Nov")
            } else {
                date_text(i, day)
            };
            let clean_date = if is(&unreadable) {
                date.clone()
            } else {
                format!("2026-11-{day:02}")
            };
            let price_raw = if is(&empties) {
                String::new()
            } else {
                price_text(i / 4, price)
            };
            let product_raw = if is(&trailing) {
                format!("{product} ")
            } else {
                product.to_string()
            };
            let clean_market = if kisenyi { "Kisenyi" } else { market.trim() };
            Record {
                raw: vec![id.clone(), date, market.clone(), product_raw, price_raw],
                clean: vec![
                    id,
                    clean_date,
                    clean_market.to_string(),
                    product.to_string(),
                    if is(&empties) {
                        String::new()
                    } else {
                        price.to_string()
                    },
                ],
                special: order[..first_ordinary].contains(&i),
            }
        })
        .collect();

    let ordinary: Vec<usize> = (0..records.len())
        .filter(|&i| !records[i].special)
        .collect();
    let mut sources: Vec<usize> = Vec::new();
    while sources.len() < spec.duplicates {
        let s = ordinary[rng.below(ordinary.len())];
        if !sources.contains(&s) {
            sources.push(s);
        }
    }
    let mut raw_rows: Vec<Vec<String>> = records.iter().map(|r| r.raw.clone()).collect();
    let mut expected: Vec<Vec<String>> = records.iter().map(|r| r.clean.clone()).collect();
    sources.sort_unstable_by(|a, b| b.cmp(a));
    for s in sources {
        // Copy the row to a spot a few rows after the original.
        let at = (s + 1 + rng.below(20)).min(raw_rows.len());
        raw_rows.insert(at, records[s].raw.clone());
    }
    records.clear();

    let mut csv = String::from("id,date,market,product,price\n");
    for r in &raw_rows {
        let cells: Vec<String> = r
            .iter()
            .map(|c| {
                if c.contains(',') {
                    format!("\"{c}\"")
                } else {
                    c.clone()
                }
            })
            .collect();
        csv.push_str(&cells.join(","));
        csv.push('\n');
    }

    let mut space_cells = 0;
    let mut wrong_cells = 0;
    let mut spellings = std::collections::HashSet::new();
    for r in &raw_rows {
        spellings.extend(
            r[2].to_lowercase()
                .starts_with("kis")
                .then(|| r[2].as_str()),
        );
        let date_wrong = r[1].len() != 10 || r[1].as_bytes()[4] != b'-';
        let price_wrong = r[4].is_empty() || r[4].parse::<usize>().is_err();
        let flags = [false, date_wrong, false, false, price_wrong];
        for (c, cell) in r.iter().enumerate() {
            let spaced = cell.trim().len() != cell.len();
            space_cells += usize::from(spaced);
            wrong_cells += usize::from(spaced || flags[c]);
        }
    }
    let rows_in = raw_rows.len();
    // Duplicates repeat rows that are the same after cleaning, so removing them leaves the base rows.
    let rows_needing_person = expected
        .iter()
        .filter(|r| r[4].is_empty() || r[1].len() != 10)
        .count();
    expected.shrink_to_fit();
    Fixture {
        csv,
        rows_in,
        rows_out: expected.len(),
        duplicate_rows: rows_in - expected.len(),
        empty_prices: spec.empty_prices,
        unreadable_dates: spec.unreadable_dates,
        rows_needing_person,
        space_cells,
        wrong_cells,
        market_spellings_seen: spellings.len(),
        expected,
    }
}
