//! Finding names that are probably the same thing spelled differently.

use std::collections::HashMap;

use kh_core::Table;

use crate::steps::SpellingGroup;

fn key(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// True when one letter added, dropped or changed turns `a` into `b`.
fn one_edit_apart(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let (ra, rb) = (&a[prefix..], &b[prefix..]);
    match ra.len().cmp(&rb.len()) {
        std::cmp::Ordering::Equal => ra[1..] == rb[1..],
        std::cmp::Ordering::Less => ra == &rb[1..],
        std::cmp::Ordering::Greater => &ra[1..] == rb,
    }
}

/// Comparing every pair is quadratic. A column with more distinct names than this is not a list of
/// categories, so only case and spacing are grouped.
const MAX_NEAR_CLUSTERS: usize = 2_000;

struct Cluster {
    key: String,
    count: usize,
    spellings: Vec<(String, usize)>,
}

/// Groups spellings by case, spacing and punctuation, then joins a group to another that is one letter
/// away. A group that is one letter away from two or more others is ambiguous and is left alone.
/// Short names (under five letters) only group by case and spacing.
pub fn suggest_spelling_groups(table: &Table, column: usize) -> Vec<SpellingGroup> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for r in 0..table.row_count() {
        if let Some(v) = table.cell(r, column).filter(|v| !v.trim().is_empty()) {
            *counts.entry(v).or_default() += 1;
        }
    }
    let mut by_key: HashMap<String, Cluster> = HashMap::new();
    for (value, n) in counts {
        let k = key(value);
        let c = by_key.entry(k.clone()).or_insert_with(|| Cluster {
            key: k,
            count: 0,
            spellings: Vec::new(),
        });
        c.count += n;
        c.spellings.push((value.to_owned(), n));
    }
    let mut clusters: Vec<Cluster> = by_key.into_values().collect();
    clusters.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));

    let near = |a: &Cluster, b: &Cluster| {
        a.key.chars().count() >= 5 && b.key.chars().count() >= 5 && one_edit_apart(&a.key, &b.key)
    };
    let mut target: Vec<usize> = (0..clusters.len()).collect();
    let compared = if clusters.len() > MAX_NEAR_CLUSTERS {
        0
    } else {
        clusters.len()
    };
    for i in 0..compared {
        let close: Vec<usize> = (0..clusters.len())
            .filter(|&j| j != i && near(&clusters[i], &clusters[j]))
            .collect();
        if let [j] = close[..]
            && j < i
            && target[j] == j
        {
            target[i] = j;
        }
    }

    let mut joined: Vec<Vec<(String, usize)>> = vec![Vec::new(); clusters.len()];
    for (cluster, root) in clusters.into_iter().zip(target) {
        joined[root].extend(cluster.spellings);
    }
    let mut groups = Vec::new();
    for mut members in joined {
        if members.len() < 2 {
            continue;
        }
        members.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        groups.push(SpellingGroup {
            canonical: members[0].0.clone(),
            variants: members.into_iter().map(|m| m.0).collect(),
        });
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::TableBuilder;

    fn column(values: &[&str]) -> Table {
        let mut b = TableBuilder::new(&["market"]).unwrap();
        for v in values {
            b.push_row(&[v]).unwrap();
        }
        b.finish()
    }

    #[test]
    fn groups_case_spacing_and_one_letter_slips() {
        let t = column(&[
            "Kisenyi", "Kisenyi", "Kisenyi", "kisenyi", "KISENYI", "Kisenyi ", "Kisenyii", "Owino",
            "Owino",
        ]);
        let g = suggest_spelling_groups(&t, 0);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].canonical, "Kisenyi");
        assert_eq!(g[0].variants.len(), 5);
    }

    #[test]
    fn ambiguous_names_are_left_alone() {
        let t = column(&[
            "Nakaseero",
            "Nakaseero",
            "Nakasero",
            "Nakasera",
            "Nakasera",
            "Nakasera",
        ]);
        // "Nakaseero" and "Nakasera" are each one letter from "Nakasero": Nakasero is ambiguous.
        let g = suggest_spelling_groups(&t, 0);
        assert!(
            g.iter()
                .all(|g| !g.variants.contains(&"Nakasero".to_owned()))
        );
    }

    #[test]
    fn short_names_only_group_by_case() {
        let g = suggest_spelling_groups(&column(&["Kla", "Kal", "kla"]), 0);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].variants.len(), 2);
    }

    #[test]
    fn a_column_of_thousands_of_names_still_groups_by_case_quickly() {
        let mut values: Vec<String> = (0..20_000).map(|i| format!("customer{i:05}")).collect();
        values.push("CUSTOMER00001".to_owned());
        let refs: Vec<&str> = values.iter().map(String::as_str).collect();
        let started = std::time::Instant::now();
        let groups = suggest_spelling_groups(&column(&refs), 0);
        assert_eq!(groups.len(), 1);
        assert!(started.elapsed().as_secs() < 5);
    }
}
