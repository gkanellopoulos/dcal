use rand::Rng;

const PROJECT_PREFIX: &str = "proj_";
const SESSION_PREFIX: &str = "sess_";
const HEX_BYTES: usize = 3;

/// Generate a project ID: `proj_` prefix + 6 random hex characters.
pub fn generate_project_id() -> String {
    prefixed_hex_id(PROJECT_PREFIX)
}

/// Generate a dcal session ID: `sess_` prefix + 6 random hex characters.
pub fn generate_session_id() -> String {
    prefixed_hex_id(SESSION_PREFIX)
}

fn prefixed_hex_id(prefix: &str) -> String {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; HEX_BYTES];
    rng.fill(&mut bytes);

    let mut id = String::with_capacity(prefix.len() + HEX_BYTES * 2);
    id.push_str(prefix);
    for byte in &bytes {
        id.push_str(&format!("{byte:02x}"));
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn project_id_has_correct_prefix() {
        let id = generate_project_id();
        assert!(id.starts_with("proj_"));
    }

    #[test]
    fn session_id_has_correct_prefix() {
        let id = generate_session_id();
        assert!(id.starts_with("sess_"));
    }

    #[test]
    fn project_id_has_correct_length() {
        let id = generate_project_id();
        // "proj_" (5) + 6 hex chars = 11
        assert_eq!(id.len(), 11);
    }

    #[test]
    fn session_id_has_correct_length() {
        let id = generate_session_id();
        // "sess_" (5) + 6 hex chars = 11
        assert_eq!(id.len(), 11);
    }

    #[test]
    fn hex_suffix_is_valid() {
        let id = generate_project_id();
        let hex_part = &id[5..];
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hex_suffix_is_lowercase() {
        let id = generate_project_id();
        let hex_part = &id[5..];
        assert_eq!(hex_part, hex_part.to_lowercase());
    }

    #[test]
    fn ids_are_unique() {
        let ids: HashSet<String> = (0..100).map(|_| generate_project_id()).collect();
        assert_eq!(ids.len(), 100);
    }
}
