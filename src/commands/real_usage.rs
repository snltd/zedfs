use crate::zfs_output;
use anyhow::{Context, ensure};
use byte_unit::Byte;

#[derive(Debug, PartialEq)]
struct Dataset {
    byte_size: u64,
    format_size: String,
    name: String,
}

pub fn run() -> anyhow::Result<()> {
    let raw_usage = zfs_output!("list", "-t", "all", "-Ho", "name,used,usedbydataset")?;
    let datasets = parse_list_output(&raw_usage).context("failed to parse ZFS output")?;
    println!("{}", format_output(datasets));
    Ok(())
}

fn parse_list_output(raw: &str) -> anyhow::Result<Vec<Dataset>> {
    let mut non_zero_datasets: Vec<Dataset> = raw
        .lines()
        .filter_map(|l| parse_dataset_line(l).ok())
        .collect();

    non_zero_datasets.sort_by_key(|dataset| dataset.byte_size);
    Ok(non_zero_datasets)
}

fn parse_dataset_line(line: &str) -> anyhow::Result<Dataset> {
    let chunks: Vec<&str> = line.split_whitespace().collect();

    ensure!(chunks.len() == 3, "failed to parse ZFS output: {line}");

    let size = if chunks[2] == "-" {
        chunks[1]
    } else {
        chunks[2]
    };

    Ok(Dataset {
        byte_size: Byte::parse_str(size, true)?.as_u64(),
        format_size: size.to_string(),
        name: chunks[0].to_string(),
    })
}

fn format_output(sorted_dataset_list: Vec<Dataset>) -> String {
    sorted_dataset_list
        .iter()
        .map(|d| format!("  {:>6}  {}\n", d.format_size, d.name))
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_dataset_line() {
        assert_eq!(
            Dataset {
                byte_size: 6050000000_u64,
                format_size: "6.05G".to_string(),
                name: "rpool/zones/serv-build/ROOT/zbe-3".to_string(),
            },
            parse_dataset_line("rpool/zones/serv-build/ROOT/zbe-3       6.13G   6.05G").unwrap()
        );
    }

    #[test]
    fn test_parse_dataset_line_wrong_chunks() {
        let result = parse_dataset_line("");

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("failed to parse ZFS output")
        );

        let result = parse_dataset_line("test/dataset 10G");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("failed to parse ZFS output")
        );
    }

    #[test]
    fn test_parse_dataset_line_invalid_byte_size() {
        let result = parse_dataset_line("test/dataset 10G_INVALID -");
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("ValueIncorrect") || err_msg.contains("character"));
    }
}
