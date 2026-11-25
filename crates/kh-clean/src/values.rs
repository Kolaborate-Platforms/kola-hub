//! Reading dates and numbers out of messy text. Nothing here guesses: unreadable text gives `None`.

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

fn month_from_name(word: &str) -> Option<u32> {
    let w = word.to_ascii_lowercase();
    let i = MONTHS.iter().position(|m| {
        *m == w || (w.len() == 3 && m.starts_with(&w)) || (w == "sept" && *m == "september")
    })?;
    Some(i as u32 + 1)
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn number(token: &str) -> Option<u32> {
    if token.is_empty() || token.len() > 4 || !token.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    token.parse().ok()
}

fn day_number(token: &str) -> Option<u32> {
    let t = token.to_ascii_lowercase();
    let t = ["st", "nd", "rd", "th"]
        .iter()
        .find_map(|s| t.strip_suffix(s))
        .unwrap_or(&t);
    number(t)
}

/// Reads a date and returns it as `YYYY-MM-DD`.
///
/// Understands `12/11/2026`, `12-11-2026`, `12.11.2026`, `12 Nov 2026`, `12 November 2026`,
/// `Nov 12, 2026` and `2026-11-13`. A date with no year (`14-Nov`) or a two digit year is not read,
/// because the year would have to be guessed. With `day_first` false, `12/11/2026` is 11 December.
pub fn parse_date(text: &str, day_first: bool) -> Option<String> {
    let tokens: Vec<&str> = text
        .split(['/', '-', '.', ' ', ','])
        .filter(|t| !t.is_empty())
        .collect();
    let [a, b, c] = tokens[..] else {
        return None;
    };
    let (year, month, day) = if a.len() == 4 && number(a).is_some() {
        (number(a)?, number(b)?, number(c)?)
    } else if c.len() == 4 {
        let year = number(c)?;
        if let Some(m) = month_from_name(b) {
            (year, m, day_number(a)?)
        } else if let Some(m) = month_from_name(a) {
            (year, m, day_number(b)?)
        } else if day_first {
            (year, number(b)?, number(a)?)
        } else {
            (year, number(a)?, number(b)?)
        }
    } else {
        return None;
    };
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) || year < 1000 {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// True when the text is already a real `YYYY-MM-DD` date.
pub fn is_clean_date(text: &str) -> bool {
    text.len() == 10 && text.contains('-') && parse_date(text, true).as_deref() == Some(text)
}

/// The only words and signs that may sit around a number. Anything else makes the cell one for a
/// person: dropping an unknown word would turn `Plot 12` or `5 km` into a bare number.
/// Longer spellings come first so that `ushs` is not read as `ush`.
const CURRENCY_MARKS: [&str; 19] = [
    "ushs.", "ushs", "ush.", "ush", "shs.", "shs", "sh.", "sh", "ugx", "kes", "tzs", "rwf", "usd",
    "$", "€", "£", "¥", "/=", "/-",
];

fn strip_currency(text: &str) -> String {
    let mut t = text.trim().to_ascii_lowercase();
    loop {
        let rest = CURRENCY_MARKS
            .iter()
            .find_map(|m| t.strip_prefix(m).or_else(|| t.strip_suffix(m)));
        match rest {
            Some(rest) => t = rest.trim().to_owned(),
            None => return t,
        }
    }
}

/// Reads a number, dropping the currency marks above (`UGX 3,500`, `3500 shs`, `$12`) and thousands
/// commas. Returns plain digits with an optional minus sign and decimal point. Commas must sit in
/// groups of three, so `1,2` is not read.
pub fn parse_number(text: &str) -> Option<String> {
    let stripped = strip_currency(text);
    let t = stripped.as_str();
    let (neg, t) = match t.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, t),
    };
    // A mark may follow the sign, as in `-$4`.
    let unmarked = strip_currency(t);
    let t = unmarked.as_str();
    let (whole, frac) = match t.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (t, None),
    };
    if whole.is_empty()
        || frac.is_some_and(|f| f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let digits = if whole.contains(',') {
        let groups: Vec<&str> = whole.split(',').collect();
        let ok = (1..=3).contains(&groups[0].len())
            && groups[1..].iter().all(|g| g.len() == 3)
            && groups.iter().all(|g| g.bytes().all(|b| b.is_ascii_digit()));
        if !ok {
            return None;
        }
        groups.concat()
    } else if whole.bytes().all(|b| b.is_ascii_digit()) {
        whole.to_owned()
    } else {
        return None;
    };
    let sign = if neg { "-" } else { "" };
    Some(match frac {
        Some(f) => format!("{sign}{digits}.{f}"),
        None => format!("{sign}{digits}"),
    })
}

/// True when the text is already a plain number such as `3500` or `-12.5`.
pub fn is_clean_number(text: &str) -> bool {
    !text.contains(',') && parse_number(text).as_deref() == Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_supported_date_shapes() {
        for (text, want) in [
            ("12/11/2026", "2026-11-12"),
            ("12 Nov 2026", "2026-11-12"),
            ("2026-11-13", "2026-11-13"),
            ("13 November 2026", "2026-11-13"),
            ("Nov 12, 2026", "2026-11-12"),
            ("1st Sept 2026", "2026-09-01"),
            (" 29/02/2028 ", "2028-02-29"),
        ] {
            assert_eq!(parse_date(text, true).as_deref(), Some(want), "{text}");
        }
    }

    #[test]
    fn day_first_can_be_switched() {
        assert_eq!(
            parse_date("12/11/2026", false).as_deref(),
            Some("2026-12-11")
        );
        assert_eq!(parse_date("13/11/2026", false), None);
    }

    #[test]
    fn dates_that_would_need_a_guess_are_not_read() {
        for text in [
            "14-Nov",
            "14/11/26",
            "31/02/2026",
            "29/02/2027",
            "",
            "soon",
            "0/1/2026",
        ] {
            assert_eq!(parse_date(text, true), None, "{text}");
        }
    }

    #[test]
    fn reads_numbers_with_currency_and_commas() {
        for (text, want) in [
            ("UGX 3,500", "3500"),
            ("UGX3,500", "3500"),
            ("3500 UGX", "3500"),
            ("3,500", "3500"),
            ("1,234,567.50", "1234567.50"),
            ("$12", "12"),
            ("-4", "-4"),
            (" 900 ", "900"),
        ] {
            assert_eq!(parse_number(text).as_deref(), Some(want), "{text}");
        }
    }

    #[test]
    fn odd_numbers_are_not_read() {
        for text in ["", "UGX", "1,2", "12,34,567", "12a4", "1.2.3", "abc", "--5"] {
            assert_eq!(parse_number(text), None, "{text}");
        }
    }

    #[test]
    fn words_that_are_not_money_are_never_dropped() {
        for text in [
            "Plot 12",
            "5 km",
            "12 boxes",
            "about 500",
            "ID 0012",
            "=3",
            "3500 each",
            "No. 4",
        ] {
            assert_eq!(parse_number(text), None, "{text}");
        }
        for (text, want) in [
            ("Shs. 3,500/=", "3500"),
            ("3500/-", "3500"),
            ("-$4", "-4"),
            ("UShs 900", "900"),
        ] {
            assert_eq!(parse_number(text).as_deref(), Some(want), "{text}");
        }
    }
}
