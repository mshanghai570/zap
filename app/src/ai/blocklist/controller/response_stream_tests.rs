use crate::settings::AgentProviderKind;

use super::{byop_generator_for, ByopGenerator};

#[test]
fn opencode_cli_errors_are_not_automatically_retried() {
    let error = crate::ai::api_error::AIApiError::Other(anyhow::anyhow!("CLI failed"));

    assert!(!super::is_retryable_for_provider(
        &error,
        Some(AgentProviderKind::OpenCodeCli)
    ));
    assert!(super::is_retryable_for_provider(
        &error,
        Some(AgentProviderKind::OpenAiCompatible)
    ));
    assert!(super::is_retryable_for_provider(&error, None));
}

#[test]
fn provider_kind_selects_the_matching_response_generator() {
    assert_eq!(
        byop_generator_for(AgentProviderKind::OpenCodeCli),
        ByopGenerator::OpenCodeCli
    );
    assert_eq!(
        byop_generator_for(AgentProviderKind::OpenAiCompatible),
        ByopGenerator::Http
    );
}
