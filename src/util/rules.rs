/// Checks if any of the given wildcard rules matches any of the given items. Used as a filter,
/// so it's a negative match.
///
pub fn omit_rules_match(item: &str, rules: &[String]) -> bool {
    !rules.iter().any(|rule| match rule.as_str() {
        r if r.starts_with('*') && r.ends_with('*') => item.contains(&r[1..r.len() - 1]),
        r if r.starts_with('*') => item.ends_with(&r[1..]),
        r if r.ends_with('*') => item.starts_with(&r[..r.len() - 1]),
        r => item == r,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let rules = vec!["whole".to_string()];
        assert!(!omit_rules_match("whole", &rules));
        assert!(omit_rules_match("empty", &rules));
    }

    #[test]
    fn test_prefix_match() {
        let rules = vec!["start*".to_string()];
        assert!(!omit_rules_match("start_of_string", &rules));
        assert!(omit_rules_match("dont_start", &rules));
    }

    #[test]
    fn test_suffix_match() {
        let rules = vec!["*end".to_string()];
        assert!(!omit_rules_match("this_is_the_end", &rules));
        assert!(omit_rules_match("end_the_matching", &rules));
    }

    #[test]
    fn test_contains_match() {
        let rules = vec!["*contains*".to_string()];
        assert!(!omit_rules_match("this_contains_a_match", &rules));
        assert!(omit_rules_match("this_does_not", &rules));
    }

    #[test]
    fn test_multiple_rules() {
        let rules = vec![
            "whole".to_string(),
            "start*".to_string(),
            "*end".to_string(),
            "*contains*".to_string(),
        ];
        assert!(!omit_rules_match("whole", &rules));
        assert!(!omit_rules_match("start_matching", &rules));
        assert!(!omit_rules_match("a_bad_end", &rules));
        assert!(!omit_rules_match("this_contains_a_match", &rules));
        assert!(omit_rules_match("nothing_matches", &rules));
    }

    #[test]
    fn test_empty_rules() {
        let rules: Vec<String> = vec![];
        assert!(omit_rules_match("anything", &rules));
    }
}
