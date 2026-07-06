use super::command::File;
use jiff::Timestamp;
use owo_colors::OwoColorize;
use regex::Regex;
use std::io::{self, Write};

type UserChoice = Option<(usize, Option<String>)>;

pub fn print_options(original_file: Option<File>, candidates: &[File]) {
    let mut stdout = io::stdout();

    for (index, candidate) in candidates.iter().enumerate() {
        match basic_line(index, candidate) {
            Ok(line) => writeln!(
                stdout,
                "{}",
                decorated_line(&original_file, candidate, line)
            )
            .expect("cannot write to console"),
            Err(e) => tracing::error!("error generating line: {e}"),
        }
    }
}

pub fn get_choice() -> anyhow::Result<String> {
    print!("choose file to promote [add 'd' for diff, 'k' to keep] > ");
    io::stdout().flush().unwrap();
    let mut buffer = String::new();
    let stdin = io::stdin();
    stdin.read_line(&mut buffer)?;
    Ok(buffer.to_owned().trim().to_string())
}

pub fn parse_choice(input: &str) -> UserChoice {
    let pattern = Regex::new(r"^(\d+)([a-z]?)$").expect("failed to compile parse_choice regex");
    let captures = pattern.captures(input)?;

    let number = captures.get(1)?.as_str().parse::<usize>().ok()?;
    let command = captures
        .get(2)
        .filter(|m| !m.as_str().is_empty())
        .map(|m| m.as_str().to_string());

    Some((number, command))
}

fn basic_line(index: usize, candidate: &File) -> anyhow::Result<String> {
    Ok(format!(
        "{:>2} {:<20} {:<35} {}",
        index,
        candidate.snapname,
        Timestamp::from_second(candidate.mtime)?
            .strftime("%Y-%m-%d %H:%M:%S %z")
            .to_string(),
        candidate.size
    ))
}

fn decorated_line(
    original_file: &Option<File>,
    candidate_file: &File,
    basic_line: String,
) -> String {
    if let Some(f) = original_file {
        if f.size == candidate_file.size && f.mtime == candidate_file.mtime {
            return basic_line.strikethrough().to_string();
        } else if f.size == candidate_file.size {
            return basic_line;
        }
        basic_line.blue().to_string()
    } else {
        basic_line
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use camino::Utf8PathBuf;

    #[test]
    fn test_basic_line() {
        let candidate = File {
            snapname: "may".to_string(),
            path: Utf8PathBuf::from("some/path"),
            mtime: 1730563919,
            size: 150679,
        };

        assert_eq!(
            " 0 may                  2024-11-02 16:11:59 +0000           150679".to_string(),
            basic_line(0, &candidate).unwrap()
        );
    }

    #[test]
    fn test_parse_choice() {
        assert_eq!(None, parse_choice("x"));
        assert_eq!(
            (47_usize, Some("k".to_string())),
            parse_choice("47k").unwrap()
        );
        assert_eq!((7_usize, None), parse_choice("7").unwrap());
    }
}
