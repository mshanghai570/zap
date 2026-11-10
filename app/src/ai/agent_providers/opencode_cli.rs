use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitStatus;

use command::r#async::Command;
use command::Stdio;

pub struct OpenCodeRunConfig {
    pub executable: PathBuf,
    pub model_id: String,
    pub working_directory: PathBuf,
    pub prompt: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenCodeCliError {
    #[error("Failed to start OpenCode CLI at {executable}: {source}")]
    Spawn {
        executable: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to collect OpenCode CLI output: {source}")]
    Output {
        #[source]
        source: std::io::Error,
    },
    #[error("OpenCode CLI exited with status {status}: {stderr}")]
    NonZeroExit { status: ExitStatus, stderr: String },
    #[error("OpenCode CLI run was cancelled")]
    Cancelled,
}

pub fn opencode_args(model_id: &str, prompt: &str) -> Vec<OsString> {
    [
        OsString::from("run"),
        OsString::from("--model"),
        OsString::from(model_id),
        OsString::from(prompt),
    ]
    .into()
}

pub async fn run_opencode(
    config: OpenCodeRunConfig,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<String, OpenCodeCliError> {
    let OpenCodeRunConfig {
        executable,
        model_id,
        working_directory,
        prompt,
    } = config;

    let mut command = Command::new(&executable);
    command
        .args(opencode_args(&model_id, &prompt))
        .current_dir(working_directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = command
        .spawn()
        .map_err(|source| OpenCodeCliError::Spawn { executable, source })?;

    let output = tokio::select! {
        _ = cancellation_rx => return Err(OpenCodeCliError::Cancelled),
        result = child.output() => result.map_err(|source| OpenCodeCliError::Output { source })?,
    };

    if !output.status.success() {
        return Err(OpenCodeCliError::NonZeroExit {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
#[path = "opencode_cli_tests.rs"]
mod tests;
