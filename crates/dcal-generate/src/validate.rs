/// Result of validating a generated CLAUDE.md.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub pass: bool,
    pub warnings: Vec<String>,
}

const REQUIRED_SECTIONS: &[&str] = &[
    "## Goals",
    "## Stack",
    "## Architecture",
    "## Working Conventions",
    "## Current Phase",
    "## Open Questions",
    "## Do Not Do",
];

const MIN_LENGTH: usize = 200;

/// Run Stage 4: validate a generated CLAUDE.md.
///
/// Checks for required sections and minimum length. Warnings do not
/// block — they are shown to the user.
pub fn run(content: &str) -> ValidationResult {
    let mut warnings = Vec::new();

    if content.len() < MIN_LENGTH {
        warnings.push(format!(
            "CLAUDE.md is short ({} chars, expected at least {MIN_LENGTH})",
            content.len()
        ));
    }

    for section in REQUIRED_SECTIONS {
        if !content.contains(section) {
            warnings.push(format!("missing section: {section}"));
        }
    }

    if !content.starts_with('#') {
        warnings.push("CLAUDE.md should start with a heading".to_string());
    }

    ValidationResult {
        pass: warnings.is_empty(),
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CLAUDE_MD: &str = "\
# my-project

A tool that does things.

## Goals

- Do the thing

## Stack

- Rust

## Architecture

TBD

## Working Conventions

- Conventional commits

## Current Phase

Ideation

## Open Questions

- None yet

## Do Not Do

- No GUI
";

    #[test]
    fn valid_document_passes() {
        let result = run(VALID_CLAUDE_MD);
        assert!(result.pass);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn missing_section_warns() {
        let content = VALID_CLAUDE_MD.replace("## Do Not Do", "## Other");
        let result = run(&content);
        assert!(!result.pass);
        assert!(result.warnings.iter().any(|w| w.contains("## Do Not Do")));
    }

    #[test]
    fn too_short_warns() {
        let result = run("# Title\n\nShort.");
        assert!(!result.pass);
        assert!(result.warnings.iter().any(|w| w.contains("short")));
    }

    #[test]
    fn no_heading_warns() {
        let content = VALID_CLAUDE_MD.trim_start_matches('#');
        let result = run(content);
        assert!(result.warnings.iter().any(|w| w.contains("heading")));
    }

    #[test]
    fn multiple_warnings_accumulated() {
        let result = run("no heading, too short");
        assert!(result.warnings.len() >= 2);
    }

    #[test]
    fn all_sections_checked() {
        let result = run("# Title\n\nSome padding text that is long enough to pass the minimum length check if we write enough words here to get over two hundred characters of content which should be sufficient for the length validator to not complain about it being too short.");
        let missing_count = result.warnings.iter().filter(|w| w.contains("missing section")).count();
        assert_eq!(missing_count, REQUIRED_SECTIONS.len());
    }
}
