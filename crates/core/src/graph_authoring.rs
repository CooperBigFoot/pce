//! authoring_lints : CriterionCommand → ConservativeArtifactReference*
//!
//! This module extracts only literal repository-relative paths whose repository meaning is stable.

/// One conservatively extracted artifact reference.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConservativeArtifactReference {
    repository_index: Option<usize>,
    path: String,
}

impl ConservativeArtifactReference {
    /// Return the one-based `$PCE_WORKTREE_N` repository index, if explicitly named.
    pub const fn repository_index(&self) -> Option<usize> {
        self.repository_index
    }
    /// Return the normalized repository-relative path.
    pub fn path(&self) -> &str {
        &self.path
    }
}

fn normalize_path(raw: &str) -> Option<String> {
    let raw = raw.strip_prefix("./").unwrap_or(raw);
    if raw.is_empty()
        || raw.starts_with('/')
        || raw.contains("://")
        || raw.starts_with('-')
        || raw
            .chars()
            .any(|character| "*?[]{}$`|;&<>()!\\".contains(character))
    {
        return None;
    }
    if raw
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return None;
    }
    if !raw.contains('/')
        && !raw
            .rsplit_once('.')
            .is_some_and(|(stem, suffix)| !stem.is_empty() && !suffix.is_empty())
    {
        return None;
    }
    Some(raw.to_owned())
}

/// Extract high-confidence literal repository-relative paths from a shell command.
pub fn extract_conservative_artifact_references(
    command: &str,
) -> Vec<ConservativeArtifactReference> {
    let mut references = Vec::new();
    for raw_token in command.split_ascii_whitespace() {
        let token = if raw_token.len() >= 2
            && ((raw_token.starts_with('\'') && raw_token.ends_with('\''))
                || (raw_token.starts_with('"') && raw_token.ends_with('"')))
        {
            &raw_token[1..raw_token.len() - 1]
        } else {
            raw_token
        };
        if let Some(rest) = token.strip_prefix("$PCE_WORKTREE_") {
            let Some((raw_index, raw_path)) = rest.split_once('/') else {
                continue;
            };
            let Ok(index) = raw_index.parse::<usize>() else {
                continue;
            };
            if index == 0 {
                continue;
            }
            if let Some(path) = normalize_path(raw_path) {
                references.push(ConservativeArtifactReference {
                    repository_index: Some(index),
                    path,
                });
            }
        } else if let Some(path) = normalize_path(token) {
            references.push(ConservativeArtifactReference {
                repository_index: None,
                path,
            });
        }
    }
    references.sort();
    references.dedup();
    references
}

/// Normalize a title for conservative equality comparison.
pub fn normalize_act_title(title: &str) -> String {
    title
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Return whether normalized titles make the same act claim with high confidence.
pub fn titles_conservatively_overlap(left: &str, right: &str) -> bool {
    let left = normalize_act_title(left);
    let right = normalize_act_title(right);
    if left == right {
        return !left.is_empty();
    }
    let left_words = left
        .split_whitespace()
        .collect::<std::collections::BTreeSet<_>>();
    let right_words = right
        .split_whitespace()
        .collect::<std::collections::BTreeSet<_>>();
    let shorter = left_words.len().min(right_words.len());
    shorter >= 3 && left_words.intersection(&right_words).count() == shorter
}

#[cfg(test)]
mod tests {
    use super::{
        extract_conservative_artifact_references, normalize_act_title,
        titles_conservatively_overlap,
    };
    #[test]
    fn ignores_tokens_with_unstable_shell_or_location_meaning() {
        let refs = extract_conservative_artifact_references(
            "cat ok/file.json /abs ../up *.json https://x/a $VAR/a '$PCE_WORKTREE_2/good/file.json'",
        );
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].path(), "ok/file.json");
        assert_eq!(refs[0].repository_index(), None);
        assert_eq!(refs[1].path(), "good/file.json");
        assert_eq!(refs[1].repository_index(), Some(2));
    }
    #[test]
    fn title_normalization_is_punctuation_and_case_only() {
        assert_eq!(
            normalize_act_title("Create-shared Manifest!"),
            "create shared manifest"
        );
        assert!(titles_conservatively_overlap(
            "Create shared manifest",
            "Create the shared manifest safely"
        ));
        assert!(!titles_conservatively_overlap(
            "Create manifest",
            "Read manifest"
        ));
    }
}
