use crate::util::zfs_info;
use anyhow::Context;
use regex::Regex;

const EXPECTED_SNAP_NAMES: [&str; 19] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
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

pub fn run() -> anyhow::Result<bool> {
    let all_snapshots = zfs_info::all_snapshots().context("failed to get snapshot list")?;
    let mut rogues = find_rogue_snapshots(&all_snapshots, &EXPECTED_SNAP_NAMES);

    if rogues.is_empty() {
        println!("No rogue snapshots");
    } else {
        rogues.sort();
        println!("{}", rogues.join("\n"))
    }

    Ok(true)
}

fn find_rogue_snapshots(snapshot_list: &[String], expected_list: &[&str]) -> Vec<String> {
    let regex = Regex::new(r"^[012]\d:[0-5]\d$").expect("invalid regex");

    snapshot_list
        .iter()
        .filter_map(|snap| filter_fn(snap, expected_list, &regex))
        .collect()
}

fn filter_fn(snapshot: &String, expected: &[&str], regex: &Regex) -> Option<String> {
    if let Some((fs, snap)) = snapshot.split_once("@")
        && !fs.starts_with("rpool/VARSHARE/zones")
        && !fs.starts_with("rpool/ROOT")
        && snap != "initial"
        && !(regex.is_match(snap))
        && !(expected.iter().any(|x| x == &snap))
    {
        return Some(snapshot.to_string());
    }
    None
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_find_rogue_snapshots() {
        let all_snapshots = vec![
            "rpool/ROOT@rogue".to_string(),
            "rpool@wednesday".to_string(),
            "rpool@rogue".to_string(),
            "rpool/VARSHARE/zones/zone@rogue".to_string(),
            "zones/myzone@initial".to_string(),
            "fast/zone/build/build@12:00".to_string(),
            "rpool/zones@october".to_string(),
            "fast/zone/build@99:99".to_string(),
        ];

        let defaults = ["wednesday", "october"];

        assert_eq!(
            vec![
                "rpool@rogue".to_string(),
                "fast/zone/build@99:99".to_string()
            ],
            find_rogue_snapshots(&all_snapshots, &defaults)
        );
    }
}
