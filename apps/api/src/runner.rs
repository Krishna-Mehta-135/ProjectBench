use std::{path::PathBuf, time::Duration};
use thiserror::Error;
use tokio::{process::Command, sync::watch, time::sleep};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct K6Request {
    pub script: PathBuf,
    pub output_json: PathBuf,
    pub summary_json: PathBuf,
    pub vus: u32,
    pub duration_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct K6Command {
    pub executable: &'static str,
    pub args: Vec<String>,
}

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("k6 process timed out")]
    Timeout,
    #[error("k6 process failed with exit code {0:?}")]
    NonZero(Option<i32>),
    #[error("k6 process could not start: {0}")]
    Start(#[from] std::io::Error),
    #[error("k6 process was canceled")]
    Canceled,
}

pub fn command_for(request: &K6Request) -> K6Command {
    K6Command {
        executable: "k6",
        args: vec![
            "run".into(),
            "--vus".into(),
            request.vus.to_string(),
            "--duration".into(),
            format!("{}s", request.duration_seconds),
            "--out".into(),
            format!("json={}", request.output_json.display()),
            "--summary-export".into(),
            request.summary_json.display().to_string(),
            request.script.display().to_string(),
        ],
    }
}

pub async fn execute(
    request: &K6Request,
    hard_timeout: Duration,
    mut cancellation: watch::Receiver<bool>,
) -> Result<(), RunnerError> {
    let command = command_for(request);
    let mut child = Command::new(command.executable)
        .args(command.args)
        .spawn()
        .map_err(RunnerError::Start)?;
    let status = tokio::select! {
        result = child.wait() => result.map_err(RunnerError::Start)?,
        _ = sleep(hard_timeout) => {
            let _ = child.kill().await;
            return Err(RunnerError::Timeout);
        }
        changed = cancellation.changed() => {
            if changed.is_ok() && *cancellation.borrow() {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(RunnerError::Canceled);
            }
            return Err(RunnerError::Canceled);
        }
    };
    if status.success() {
        Ok(())
    } else {
        Err(RunnerError::NonZero(status.code()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_fixed_k6_command_without_shell_text() {
        let request = K6Request {
            script: "scenario.js".into(),
            output_json: "run.jsonl".into(),
            summary_json: "summary.json".into(),
            vus: 1,
            duration_seconds: 60,
        };
        let command = command_for(&request);
        assert_eq!(command.executable, "k6");
        assert_eq!(
            command.args,
            vec![
                "run",
                "--vus",
                "1",
                "--duration",
                "60s",
                "--out",
                "json=run.jsonl",
                "--summary-export",
                "summary.json",
                "scenario.js"
            ]
        );
    }

    #[test]
    fn user_input_is_data_not_a_shell_command() {
        let request = K6Request {
            script: "$(touch compromised)".into(),
            output_json: "run.jsonl".into(),
            summary_json: "summary.json".into(),
            vus: 1,
            duration_seconds: 1,
        };
        let command = command_for(&request);
        assert!(command.args.iter().any(|arg| arg == "$(touch compromised)"));
        assert!(!command.args.iter().any(|arg| arg.contains("sh -c")));
    }
}
