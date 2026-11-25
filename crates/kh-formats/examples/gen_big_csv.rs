//! Writes a seeded, invented market-price CSV for benchmarks.
//! Usage: cargo run --release -p kh-formats --example gen_big_csv -- <rows> <output.csv> [pad_bytes]
//! `pad_bytes` adds that much unique text to every note, to make wide files (about 140 gives 200 MB per million rows).

use std::io::{BufWriter, Write};

const MARKETS: [&str; 7] = [
    "Kisenyi",
    "Owino",
    "Nakasero",
    "Kalerwe",
    "Nakawa",
    "Ndeeba",
    "Wandegeya",
];
const ITEMS: [&str; 12] = [
    "maize flour",
    "beans",
    "rice",
    "sugar",
    "cooking oil",
    "tomatoes",
    "onions",
    "matooke",
    "cassava",
    "millet",
    "groundnuts",
    "salt",
];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*, seeded so every run writes the same file.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }
}

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let rows: u64 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);
    let path = args.next().unwrap_or_else(|| "big.csv".to_string());
    let pad: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut out = BufWriter::new(std::fs::File::create(&path)?);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    writeln!(out, "record_id,date,market,item,price,collector,note")?;
    for i in 0..rows {
        let day = 1 + rng.next() % 28;
        let month = 1 + rng.next() % 12;
        let date = match rng.next() % 4 {
            0 => format!("{day:02}/{month:02}/2026"),
            1 => format!("{day} Nov 2026"),
            2 => format!("2026-{month:02}-{day:02}"),
            _ => format!("{day:02}-{month:02}-2026"),
        };
        let price = 500 + rng.next() % 9_500;
        let price_text = if price.is_multiple_of(7) {
            String::new()
        } else {
            format!("UGX {},{:03}", price / 1000, price % 1000)
        };
        let note = if rng.next().is_multiple_of(5) {
            format!(
                "checked by team {}, stall {}",
                rng.next() % 40,
                rng.next() % 900
            )
        } else {
            String::new()
        };
        let note = if pad > 0 {
            format!("{note} {}", filler(&mut rng, pad))
        } else {
            note
        };
        writeln!(
            out,
            "{i},{date},{},{},\"{price_text}\",collector-{},\"{note}\"",
            rng.pick(&MARKETS),
            rng.pick(&ITEMS),
            rng.next() % 60
        )?;
    }
    out.flush()
}

/// Unique-looking words so wide files do not compress into a few repeated values.
fn filler(rng: &mut Rng, bytes: usize) -> String {
    let mut s = String::with_capacity(bytes + 8);
    while s.len() < bytes {
        s.push_str(&format!("w{:x} ", rng.next() % 0xFF_FFFF));
    }
    s
}
