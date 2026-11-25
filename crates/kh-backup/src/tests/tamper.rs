use super::*;
use crate::header::{CHUNK, HEADER_LEN};
use crate::{Error, Limits};

const CT: usize = CHUNK + 16;

fn archive() -> Vec<u8> {
    make_archive(&[
        ("a.bin", pattern(CHUNK * 3 + 500, 4)),
        ("b.txt", b"hello".to_vec()),
    ])
}

fn damaged(bytes: &[u8]) -> bool {
    matches!(restore_bytes(bytes, Limits::default()), Err(Error::Damaged))
}

#[test]
fn untouched_archive_restores() {
    restore_bytes(&archive(), Limits::default()).expect("restore");
}

#[test]
fn every_header_bit_flip_is_refused() {
    let good = archive();
    for byte in 0..HEADER_LEN {
        for bit in 0..8 {
            let mut bad = good.clone();
            bad[byte] ^= 1 << bit;
            assert!(
                restore_bytes(&bad, Limits::default()).is_err(),
                "byte {byte} bit {bit}"
            );
        }
    }
}

#[test]
fn header_checks_name_the_right_problem() {
    let good = archive();
    let mut bad = good.clone();
    bad[0] = b'X';
    assert!(matches!(
        restore_bytes(&bad, Limits::default()),
        Err(Error::NotABackup)
    ));
    let mut bad = good.clone();
    bad[4] = 2;
    assert!(matches!(
        restore_bytes(&bad, Limits::default()),
        Err(Error::NewerVersion)
    ));
    // Out of range costs are refused outright, not tried.
    for (at, value) in [
        (5, 4_000_000_000u32),
        (5, 1000),
        (9, 0),
        (9, 11),
        (13, 0),
        (13, 5),
        (40, 4096),
    ] {
        let mut bad = good.clone();
        bad[at..at + 4].copy_from_slice(&value.to_le_bytes());
        assert!(damaged(&bad), "offset {at} value {value}");
    }
    assert!(matches!(
        restore_bytes(&good[..10], Limits::default()),
        Err(Error::NotABackup)
    ));
}

#[test]
fn a_flipped_byte_in_any_chunk_is_refused() {
    let good = archive();
    let chunks = (good.len() - HEADER_LEN).div_ceil(CT);
    assert!(chunks >= 4);
    for chunk in 0..chunks {
        let start = HEADER_LEN + chunk * CT;
        for offset in [0, 100, CT - 1] {
            let at = (start + offset).min(good.len() - 1);
            let mut bad = good.clone();
            bad[at] ^= 0x10;
            assert!(damaged(&bad), "chunk {chunk} offset {offset}");
        }
    }
}

#[test]
fn truncation_anywhere_is_refused() {
    let good = archive();
    let mut cuts = vec![
        HEADER_LEN,
        HEADER_LEN + 1,
        good.len() - 1,
        good.len() - 16,
        good.len() - 17,
    ];
    for k in 1..4 {
        cuts.push(HEADER_LEN + k * CT);
        cuts.push(HEADER_LEN + k * CT - 1);
        cuts.push(HEADER_LEN + k * CT + 1);
    }
    for cut in cuts {
        assert!(damaged(&good[..cut]), "cut at {cut}");
    }
}

#[test]
fn swapped_repeated_and_extended_chunks_are_refused() {
    let good = archive();
    let chunk = |i: usize| good[HEADER_LEN + i * CT..HEADER_LEN + (i + 1) * CT].to_vec();

    let mut swapped = good[..HEADER_LEN].to_vec();
    swapped.extend(chunk(1));
    swapped.extend(chunk(0));
    swapped.extend_from_slice(&good[HEADER_LEN + 2 * CT..]);
    assert!(damaged(&swapped));

    let mut repeated = good[..HEADER_LEN + 2 * CT].to_vec();
    repeated.extend(chunk(1));
    repeated.extend_from_slice(&good[HEADER_LEN + 2 * CT..]);
    assert!(damaged(&repeated));

    for extra in [vec![0u8], vec![0u8; 16], vec![7u8; CT], chunk(0)] {
        let mut longer = good.clone();
        longer.extend(extra);
        assert!(damaged(&longer));
    }
}
