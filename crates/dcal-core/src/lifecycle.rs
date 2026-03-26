use crate::project::ProjectStatus;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("invalid status transition: {from} → {to}")]
pub struct TransitionError {
    pub from: ProjectStatus,
    pub to: ProjectStatus,
}

/// Check whether a status transition is valid.
///
/// Valid transitions:
/// - Ideation → Active
/// - Active → Paused, Blocked, Completed
/// - Paused → Active, Archived
/// - Blocked → Active, Paused, Archived
/// - Completed → Archived, Active (reopen)
/// - Archived → (terminal, no transitions out)
pub fn can_transition(from: ProjectStatus, to: ProjectStatus) -> bool {
    use ProjectStatus::*;

    matches!(
        (from, to),
        (Ideation, Active)
            | (Active, Paused)
            | (Active, Blocked)
            | (Active, Completed)
            | (Paused, Active)
            | (Paused, Archived)
            | (Blocked, Active)
            | (Blocked, Paused)
            | (Blocked, Archived)
            | (Completed, Archived)
            | (Completed, Active)
    )
}

/// Validate and return the new status, or an error if the transition is invalid.
pub fn validate_transition(
    from: ProjectStatus,
    to: ProjectStatus,
) -> Result<ProjectStatus, TransitionError> {
    if can_transition(from, to) {
        Ok(to)
    } else {
        Err(TransitionError { from, to })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ProjectStatus::*;

    #[test]
    fn ideation_to_active() {
        assert!(can_transition(Ideation, Active));
    }

    #[test]
    fn ideation_cannot_skip_to_paused() {
        assert!(!can_transition(Ideation, Paused));
    }

    #[test]
    fn active_to_paused() {
        assert!(can_transition(Active, Paused));
    }

    #[test]
    fn active_to_blocked() {
        assert!(can_transition(Active, Blocked));
    }

    #[test]
    fn active_to_completed() {
        assert!(can_transition(Active, Completed));
    }

    #[test]
    fn paused_to_active() {
        assert!(can_transition(Paused, Active));
    }

    #[test]
    fn paused_to_archived() {
        assert!(can_transition(Paused, Archived));
    }

    #[test]
    fn paused_cannot_go_to_completed() {
        assert!(!can_transition(Paused, Completed));
    }

    #[test]
    fn blocked_to_active() {
        assert!(can_transition(Blocked, Active));
    }

    #[test]
    fn blocked_to_paused() {
        assert!(can_transition(Blocked, Paused));
    }

    #[test]
    fn blocked_to_archived() {
        assert!(can_transition(Blocked, Archived));
    }

    #[test]
    fn completed_to_archived() {
        assert!(can_transition(Completed, Archived));
    }

    #[test]
    fn completed_to_active_reopen() {
        assert!(can_transition(Completed, Active));
    }

    #[test]
    fn archived_is_terminal() {
        assert!(!can_transition(Archived, Active));
        assert!(!can_transition(Archived, Paused));
        assert!(!can_transition(Archived, Blocked));
        assert!(!can_transition(Archived, Completed));
        assert!(!can_transition(Archived, Ideation));
    }

    #[test]
    fn same_status_is_invalid() {
        assert!(!can_transition(Active, Active));
        assert!(!can_transition(Paused, Paused));
    }

    #[test]
    fn validate_returns_ok_for_valid() {
        let result = validate_transition(Active, Paused);
        assert_eq!(result.unwrap(), Paused);
    }

    #[test]
    fn validate_returns_err_for_invalid() {
        let result = validate_transition(Archived, Active);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.from, Archived);
        assert_eq!(err.to, Active);
    }

    #[test]
    fn error_message_is_readable() {
        let err = TransitionError {
            from: Paused,
            to: Completed,
        };
        assert_eq!(err.to_string(), "invalid status transition: paused → completed");
    }
}
