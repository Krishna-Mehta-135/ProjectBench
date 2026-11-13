use std::{path::PathBuf, time::Duration};
use thiserror::Error;
use tokio::{process::Command, time::timeout};

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

pub async fn execute(request: &K6Request, hard_timeout: Duration) -> Result<(), RunnerError> {
    let command = command_for(request);
    let status = timeout(
        hard_timeout,
        Command::new(command.executable).args(command.args).status(),
    )
    .await
    .map_err(|_| RunnerError::Timeout)??;
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
