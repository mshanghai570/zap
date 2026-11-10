use std::ffi::OsString;
use std::path::PathBuf;

use super::{opencode_args, run_opencode, OpenCodeCliError, OpenCodeRunConfig};

fn config(executable: &str) -> OpenCodeRunConfig {
    OpenCodeRunConfig {
        executable: PathBuf::from(executable),
        model_id: "openai/gpt-5".to_owned(),
        working_directory: std::env::current_dir().expect("current directory should be available"),
        prompt: "Say hello".to_owned(),
    }
}

#[test]
fn opencode_args_preserve_argument_boundaries() {
    let model_id = r#"provider/model with spaces "quotes" ; $(touch should-not-run)"#;
    let prompt = r#"Use "quoted text"; keep $(not-expanded) and `literal text` exactly."#;

    assert_eq!(
        opencode_args(model_id, prompt),
        vec![
            OsString::from("run"),
            OsString::from("--model"),
            OsString::from(model_id),
            OsString::from(prompt),
        ]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_opencode_returns_stdout_on_success() {
    let mut config = config("/bin/echo");
    config.prompt = "hello".to_owned();

    let (cancellation_tx, cancellation_rx) = futures::channel::oneshot::channel();

    let result = run_opencode(config, cancellation_rx).await;
    drop(cancellation_tx);
    let output = result.expect("echo should succeed");

    assert_eq!(output, "run --model openai/gpt-5 hello\n");
}

#[cfg(unix)]
#[tokio::test]
async fn run_opencode_reports_spawn_failure() {
    let config = config("/definitely/missing/opencode-cli-test-executable");
    let (cancellation_tx, cancellation_rx) = futures::channel::oneshot::channel();

    let result = run_opencode(config, cancellation_rx).await;
    drop(cancellation_tx);
    let error = result.expect_err("missing executable should fail");

    assert!(matches!(&error, OpenCodeCliError::Spawn { .. }));
    assert!(error.to_string().contains("Failed to start OpenCode CLI"));
}

#[cfg(unix)]
#[tokio::test]
async fn run_opencode_includes_stderr_for_non_zero_exit() {
    let config = config("/usr/bin/env");
    let (cancellation_tx, cancellation_rx) = futures::channel::oneshot::channel();

    let result = run_opencode(config, cancellation_rx).await;
    drop(cancellation_tx);
    let error = result.expect_err("env should fail to launch the missing run command");

    let OpenCodeCliError::NonZeroExit { stderr, .. } = &error else {
        panic!("expected a non-zero exit error");
    };
    assert!(!stderr.is_empty());
    assert!(stderr.contains("run"));
    assert!(error.to_string().contains(stderr));
}

#[cfg(unix)]
#[tokio::test]
async fn run_opencode_returns_cancellation_when_signalled() {
    let config = config("/usr/bin/yes");
    let (cancellation_tx, cancellation_rx) = futures::channel::oneshot::channel();
    cancellation_tx
        .send(())
        .expect("cancellation receiver should still be active");

    let error = run_opencode(config, cancellation_rx)
        .await
        .expect_err("cancelled runs should not return output");

    assert!(matches!(error, OpenCodeCliError::Cancelled));
}
