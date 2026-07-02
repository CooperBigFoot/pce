//! Vision creation workflow.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate};
use thiserror::Error;
use tracing::{instrument, warn};

/// A validated, trimmed, non-empty vision name as given by the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionName(String);

impl VisionName {
    /// Parse a raw user-provided name into a trimmed [`VisionName`].
    ///
    /// # Errors
    ///
    /// | Variant | When |
    /// |---|---|
    /// | [`VisionError::EmptyName`] | The input is empty or whitespace-only. |
    #[instrument]
    pub fn parse(raw: &str) -> Result<Self, VisionError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(VisionError::EmptyName);
        }

        Ok(Self(trimmed.to_owned()))
    }

    /// Return the trimmed name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VisionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A URL/dir-safe slug derived from a [`VisionName`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slug(String);

impl Slug {
    /// Derive a slug from a [`VisionName`].
    ///
    /// # Errors
    ///
    /// | Variant | When |
    /// |---|---|
    /// | [`VisionError::UnsluggableName`] | The name contains no ASCII alphanumeric characters. |
    #[instrument]
    pub fn from_name(name: &VisionName) -> Result<Self, VisionError> {
        let mut slug = String::new();
        let mut pending_separator = false;

        for ch in name.as_str().chars() {
            if ch.is_ascii_alphanumeric() {
                if pending_separator && !slug.is_empty() {
                    slug.push('-');
                }
                slug.push(ch.to_ascii_lowercase());
                pending_separator = false;
            } else {
                pending_separator = true;
            }
        }

        if slug.is_empty() {
            return Err(VisionError::UnsluggableName {
                name: name.as_str().to_owned(),
            });
        }

        Ok(Self(slug))
    }

    /// Return the slug string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The local calendar date used for the directory prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreationDate(NaiveDate);

impl CreationDate {
    /// Return today's local calendar date.
    #[instrument]
    pub fn today() -> Self {
        Self(Local::now().date_naive())
    }

    /// Parse a strict `YYYY-MM-DD` calendar date.
    ///
    /// # Errors
    ///
    /// | Variant | When |
    /// |---|---|
    /// | [`VisionError::InvalidDate`] | The input is not a valid `%Y-%m-%d` date. |
    #[instrument]
    pub fn parse(s: &str) -> Result<Self, VisionError> {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map(Self)
            .map_err(|_| VisionError::InvalidDate {
                input: s.to_owned(),
            })
    }
}

impl fmt::Display for CreationDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%d"))
    }
}

/// The path to a created or existing vision directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionDir(PathBuf);

impl VisionDir {
    /// Return the directory path.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl fmt::Display for VisionDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

/// Whether the target directory was created or already existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisionDirOutcome {
    /// The target directory and stub were created.
    Created,
    /// The target directory already existed and was left untouched.
    AlreadyExisted,
}

/// A newly requested vision directory operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewVision {
    dir: VisionDir,
    outcome: VisionDirOutcome,
}

impl NewVision {
    /// Return the vision directory.
    pub fn dir(&self) -> &VisionDir {
        &self.dir
    }

    /// Return the creation outcome.
    pub fn outcome(&self) -> VisionDirOutcome {
        self.outcome
    }
}

/// Errors from vision creation.
#[derive(Debug, Error)]
pub enum VisionError {
    /// Returned when the user-provided name is empty or whitespace-only.
    #[error("vision name cannot be empty")]
    EmptyName,

    /// Returned when slugification finds no ASCII alphanumeric characters.
    #[error("vision name cannot be slugified: {name:?}")]
    UnsluggableName {
        /// The trimmed name that could not be slugified.
        name: String,
    },

    /// Returned when a date string at the system boundary is malformed.
    #[error("invalid creation date: {input:?}")]
    InvalidDate {
        /// The invalid date input.
        input: String,
    },

    /// Returned when creating the target directory fails.
    #[error("failed to create vision directory {path}: {source}")]
    CreateDirFailed {
        /// The directory path that could not be created.
        path: PathBuf,
        /// The source I/O error.
        source: std::io::Error,
    },

    /// Returned when writing the initial vision stub fails.
    #[error("failed to write vision stub {path}: {source}")]
    WriteStubFailed {
        /// The stub path that could not be written.
        path: PathBuf,
        /// The source I/O error.
        source: std::io::Error,
    },

    /// Returned when scanning the planning root for duplicate slugs fails.
    #[error("failed to scan planning directory {path}: {source}")]
    ScanPlanningFailed {
        /// The planning root path that could not be scanned.
        path: PathBuf,
        /// The source I/O error.
        source: std::io::Error,
    },
}

/// Create a dated vision directory and write its initial stub.
///
/// # Errors
///
/// | Variant | When |
/// |---|---|
/// | [`VisionError::UnsluggableName`] | The parsed name contains no ASCII alphanumeric characters. |
/// | [`VisionError::ScanPlanningFailed`] | The planning root exists but cannot be scanned. |
/// | [`VisionError::CreateDirFailed`] | The target directory cannot be created. |
/// | [`VisionError::WriteStubFailed`] | The `vision.md` stub cannot be written. |
#[instrument(skip(planning_root))]
pub fn create_vision(
    name: &VisionName,
    planning_root: &Path,
    today: CreationDate,
) -> Result<NewVision, VisionError> {
    let slug = Slug::from_name(name)?;
    let target_name = format!("{today}-{slug}");
    let target = planning_root.join(&target_name);

    warn_for_different_date_duplicates(planning_root, &target_name, &slug)?;

    if target.is_dir() {
        warn!(
            dir = %target.display(),
            "vision directory already exists; leaving contents untouched"
        );
        return Ok(NewVision {
            dir: VisionDir(target),
            outcome: VisionDirOutcome::AlreadyExisted,
        });
    }

    fs::create_dir_all(&target).map_err(|source| VisionError::CreateDirFailed {
        path: target.clone(),
        source,
    })?;

    let stub_path = target.join("vision.md");
    fs::write(&stub_path, render_vision_stub(name)).map_err(|source| {
        VisionError::WriteStubFailed {
            path: stub_path,
            source,
        }
    })?;

    Ok(NewVision {
        dir: VisionDir(target),
        outcome: VisionDirOutcome::Created,
    })
}

/// Render the initial `vision.md` stub for a vision name.
#[instrument]
pub fn render_vision_stub(name: &VisionName) -> String {
    format!(
        "# Vision: {name}\n\n## Goal / Why\n\n## Scope — In\n\n## Scope — Out (explicit non-goals)\n\n## Constraints\n\n## Acceptance criteria (vision-level \"done\")\n\n## Decomposition hints\n\n## Open questions / risks\n"
    )
}

fn warn_for_different_date_duplicates(
    planning_root: &Path,
    target_name: &str,
    slug: &Slug,
) -> Result<(), VisionError> {
    if !planning_root.exists() {
        return Ok(());
    }

    let entries =
        fs::read_dir(planning_root).map_err(|source| VisionError::ScanPlanningFailed {
            path: planning_root.to_path_buf(),
            source,
        })?;

    for entry in entries {
        let entry = entry.map_err(|source| VisionError::ScanPlanningFailed {
            path: planning_root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| is_different_date_duplicate(name, target_name, slug))
        {
            warn!(
                existing = %path.display(),
                "slug already exists under a different date; creating a new dated directory"
            );
        }
    }

    Ok(())
}

fn is_different_date_duplicate(entry_name: &str, target_name: &str, slug: &Slug) -> bool {
    is_date_shaped_prefix(entry_name)
        && suffix_starting_after_date(entry_name) == slug.as_str()
        && entry_name != target_name
}

fn suffix_starting_after_date(entry_name: &str) -> &str {
    &entry_name[11..]
}

fn is_date_shaped_prefix(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() > 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
        && bytes[10] == b'-'
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::vision::{
        CreationDate, Slug, VisionDirOutcome, VisionError, VisionName, create_vision,
        render_vision_stub,
    };

    #[test]
    fn slugifies_examples_and_rejects_empty_slugs() {
        let cases = [
            ("auth refactor", "auth-refactor"),
            ("  Auth   Refactor!! ", "auth-refactor"),
            ("CamelCase_and_v2.1", "camelcase-and-v2-1"),
            ("Émile's plan", "mile-s-plan"),
        ];

        for (raw, expected) in cases {
            let name = VisionName::parse(raw).expect("name should parse");
            let slug = Slug::from_name(&name).expect("slug should parse");
            assert_eq!(slug.as_str(), expected);
        }

        let unsluggable_name = VisionName::parse("***").expect("name should parse");
        assert!(matches!(
            Slug::from_name(&unsluggable_name),
            Err(VisionError::UnsluggableName { name }) if name == "***"
        ));
        assert!(matches!(VisionName::parse(""), Err(VisionError::EmptyName)));
        assert!(matches!(
            VisionName::parse("   "),
            Err(VisionError::EmptyName)
        ));
    }

    #[test]
    fn renders_exact_vision_stub() {
        let name = VisionName::parse("auth refactor").expect("name should parse");

        assert_eq!(
            render_vision_stub(&name),
            "# Vision: auth refactor\n\n## Goal / Why\n\n## Scope — In\n\n## Scope — Out (explicit non-goals)\n\n## Constraints\n\n## Acceptance criteria (vision-level \"done\")\n\n## Decomposition hints\n\n## Open questions / risks\n"
        );
    }

    #[test]
    fn creates_dated_dir_and_stub() {
        let root = temp_planning_root("dir-naming");
        reset_dir(&root);

        let name = VisionName::parse("auth refactor").expect("name should parse");
        let date = CreationDate::parse("2026-07-03").expect("date should parse");
        let new_vision = create_vision(&name, &root, date).expect("vision should create");
        let expected_dir = root.join("2026-07-03-auth-refactor");

        assert_eq!(new_vision.outcome(), VisionDirOutcome::Created);
        assert_eq!(new_vision.dir().as_path(), expected_dir.as_path());
        assert!(expected_dir.is_dir());
        assert!(expected_dir.join("vision.md").is_file());

        cleanup_dir(&root);
    }

    #[test]
    fn warns_and_creates_with_different_date_duplicate() {
        let root = temp_planning_root("different-date");
        reset_dir(&root);
        let old_dir = root.join("2020-01-01-auth-refactor");
        fs::create_dir_all(&old_dir).expect("old dir should create");

        let name = VisionName::parse("auth refactor").expect("name should parse");
        let date = CreationDate::parse("2026-07-03").expect("date should parse");
        let new_vision = create_vision(&name, &root, date).expect("vision should create");
        let new_dir = root.join("2026-07-03-auth-refactor");

        assert_eq!(new_vision.outcome(), VisionDirOutcome::Created);
        assert!(new_dir.join("vision.md").is_file());
        assert!(old_dir.is_dir());
        assert_eq!(
            fs::read_dir(&old_dir).expect("old dir should read").count(),
            0
        );

        cleanup_dir(&root);
    }

    #[test]
    fn same_date_collision_does_not_overwrite() {
        let root = temp_planning_root("same-date");
        reset_dir(&root);
        let existing_dir = root.join("2026-07-03-auth-refactor");
        fs::create_dir_all(&existing_dir).expect("existing dir should create");
        let stub_path = existing_dir.join("vision.md");
        let sentinel = b"SENTINEL do not clobber\n";
        fs::write(&stub_path, sentinel).expect("sentinel should write");

        let name = VisionName::parse("auth refactor").expect("name should parse");
        let date = CreationDate::parse("2026-07-03").expect("date should parse");
        let new_vision = create_vision(&name, &root, date).expect("vision should be idempotent");

        assert_eq!(new_vision.outcome(), VisionDirOutcome::AlreadyExisted);
        assert_eq!(new_vision.dir().as_path(), existing_dir.as_path());
        assert_eq!(
            fs::read(&stub_path).expect("sentinel should read"),
            sentinel
        );
        assert_eq!(
            fs::read_dir(&existing_dir)
                .expect("existing dir should read")
                .count(),
            1
        );

        cleanup_dir(&root);
    }

    fn temp_planning_root(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("pce-core-{tag}-{pid}", pid = std::process::id()))
    }

    fn reset_dir(path: &PathBuf) {
        cleanup_dir(path);
        fs::create_dir_all(path).expect("temp dir should create");
    }

    fn cleanup_dir(path: &PathBuf) {
        let _ = fs::remove_dir_all(path);
    }
}
