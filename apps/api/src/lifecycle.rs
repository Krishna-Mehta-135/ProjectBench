use crate::RunStatus;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionError {
    #[error("cannot transition a {from:?} run to {to:?}")]
    Invalid { from: RunStatus, to: RunStatus },
}

pub fn transition(from: RunStatus, to: RunStatus) -> Result<RunStatus, TransitionError> {
    let allowed = matches!(
        (&from, &to),
        (RunStatus::Queued, RunStatus::Running)
            | (RunStatus::Queued, RunStatus::Canceled)
            | (RunStatus::Running, RunStatus::Completed)
            | (RunStatus::Running, RunStatus::Failed)
            | (RunStatus::Running, RunStatus::Canceled)
            | (RunStatus::Running, RunStatus::Aborted)
    );
    if allowed {
        Ok(to)
    } else {
        Err(TransitionError::Invalid { from, to })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permits_normal_completion() {
        assert_eq!(
            transition(RunStatus::Running, RunStatus::Completed).unwrap(),
            RunStatus::Completed
        );
    }

    #[test]
    fn prevents_terminal_state_reuse() {
        assert!(transition(RunStatus::Completed, RunStatus::Running).is_err());
    }
}
