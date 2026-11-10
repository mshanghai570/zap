//! Smoke tests for BYOP provider configuration and lookup.

use ai::LLMId;
use settings::Setting;
use warpui::{App, SingletonEntity};

use crate::ai::agent_providers::{
    build_byop_models_by_feature, build_byop_models_by_feature_for_target, llm_id, lookup_byop,
    AgentProviderSecrets,
};
use crate::ai::execution_profiles::profiles::AIExecutionProfilesModel;
use crate::ai::llms::{AvailableLLMs, DisableReason, LLMPreferences};
use crate::ai::mcp::TemplatableMCPServerManager;
use crate::auth::{AuthManager, AuthStateProvider};
use crate::cloud_object::model::persistence::ObjectStoreModel;
use crate::cloud_object::update_manager::UpdateManager;
use crate::network::NetworkStatus;
use crate::settings::{
    AISettings, AgentProvider, AgentProviderApiType, AgentProviderKind, AgentProviderModel,
    PrivacySettings,
};
use crate::test_util::settings::initialize_settings_for_tests;
use crate::workspaces::user_workspaces::UserWorkspaces;
use crate::LaunchMode;

fn sample_provider(id: &str) -> AgentProvider {
    AgentProvider {
        id: id.to_owned(),
        name: "Test Ollama".to_owned(),
        kind: Default::default(),
        api_type: AgentProviderApiType::Ollama,
        base_url: "http://localhost:11434".to_owned(),
        models: vec![AgentProviderModel::from_id("llama3.2".to_owned())],
        extra_headers: Vec::new(),
        cli_executable: None,
    }
}

fn choice_ids(available: &AvailableLLMs) -> Vec<String> {
    serde_json::to_value(available)
        .expect("available models should serialize")
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .expect("serialized models should contain choices")
        .iter()
        .map(|choice| {
            choice
                .get("id")
                .and_then(serde_json::Value::as_str)
                .expect("serialized model should contain an ID")
                .to_owned()
        })
        .collect()
}

fn init_byop_test_app(app: &mut warpui::App) {
    initialize_settings_for_tests(app);
    app.add_singleton_model(AgentProviderSecrets::new);
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(|_| AuthStateProvider::new_for_test());
    app.add_singleton_model(AuthManager::new_for_test);
    app.add_singleton_model(UpdateManager::mock);
    app.add_singleton_model(ObjectStoreModel::mock);
    app.add_singleton_model(|_| TemplatableMCPServerManager::default());
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(UserWorkspaces::default_mock);
    app.add_singleton_model(|ctx| {
        AIExecutionProfilesModel::new(&LaunchMode::new_for_unit_test(), ctx)
    });
    app.add_singleton_model(LLMPreferences::new);
}

#[test]
fn smoke_build_byop_models_by_feature_exposes_configured_models() {
    App::test((), |mut app| async move {
        init_byop_test_app(&mut app);

        let provider_id = "provider-smoke-1";
        app.update(|ctx| {
            AISettings::handle(ctx).update(ctx, |settings, ctx| {
                let _ = settings
                    .agent_providers
                    .set_value(vec![sample_provider(provider_id)], ctx);
            });
        });

        app.read(|ctx| {
            let choices: Vec<_> = LLMPreferences::as_ref(ctx)
                .get_base_llm_choices_for_agent_mode()
                .collect();
            assert_eq!(choices.len(), 1, "expected one BYOP model in picker");
            assert!(
                choices[0].disable_reason.is_none(),
                "valid provider should not be disabled"
            );
            assert_eq!(
                choices[0].id.as_str(),
                llm_id::encode(provider_id, "llama3.2").as_str()
            );
        });
    });
}

#[test]
fn smoke_build_byop_models_by_feature_uses_placeholder_when_misconfigured() {
    App::test((), |mut app| async move {
        init_byop_test_app(&mut app);

        app.read(|ctx| {
            let default = LLMPreferences::as_ref(ctx).get_default_base_model();
            assert_eq!(
                default.disable_reason,
                Some(DisableReason::Unavailable),
                "empty config should surface placeholder entry"
            );
        });
    });
}

#[test]
fn smoke_build_byop_models_by_feature_skips_empty_base_url() {
    App::test((), |mut app| async move {
        init_byop_test_app(&mut app);

        app.update(|ctx| {
            AISettings::handle(ctx).update(ctx, |settings, ctx| {
                let mut broken = sample_provider("broken");
                broken.base_url.clear();
                let _ = settings.agent_providers.set_value(vec![broken], ctx);
            });
        });

        app.read(|ctx| {
            let default = LLMPreferences::as_ref(ctx).get_default_base_model();
            assert_eq!(
                default.disable_reason,
                Some(DisableReason::Unavailable),
                "provider with empty base_url must not appear as selectable model"
            );
        });
    });
}

#[test]
fn opencode_cli_models_are_agent_mode_only_and_intel_macos_only() {
    App::test((), |mut app| async move {
        init_byop_test_app(&mut app);

        let http_provider = sample_provider("http-provider");
        let mut opencode_provider = sample_provider("opencode-provider");
        opencode_provider.name = "OpenCode".to_owned();
        opencode_provider.kind = AgentProviderKind::OpenCodeCli;
        opencode_provider.base_url.clear();
        opencode_provider.models = vec![AgentProviderModel::from_id("openai/gpt-5".to_owned())];

        app.update(|ctx| {
            AISettings::handle(ctx).update(ctx, |settings, ctx| {
                let _ = settings
                    .agent_providers
                    .set_value(vec![http_provider, opencode_provider], ctx);
            });
        });

        let http_id = llm_id::encode("http-provider", "llama3.2");
        let opencode_id = llm_id::encode("opencode-provider", "openai/gpt-5");
        app.read(|ctx| {
            let intel_models = build_byop_models_by_feature_for_target(ctx, true);
            let agent_mode_ids = choice_ids(&intel_models.agent_mode);
            assert!(agent_mode_ids.contains(&opencode_id.to_string()));
            assert!(agent_mode_ids.contains(&http_id.to_string()));
            for choices in [
                choice_ids(&intel_models.coding),
                choice_ids(intel_models.cli_agent.as_ref().expect("CLI models exist")),
                choice_ids(
                    intel_models
                        .computer_use
                        .as_ref()
                        .expect("computer-use models exist"),
                ),
            ] {
                assert!(choices.contains(&http_id.to_string()));
                assert!(!choices.contains(&opencode_id.to_string()));
            }

            let non_intel_models = build_byop_models_by_feature_for_target(ctx, false);
            let non_intel_agent_mode_ids = choice_ids(&non_intel_models.agent_mode);
            assert!(non_intel_agent_mode_ids.contains(&http_id.to_string()));
            assert!(!non_intel_agent_mode_ids.contains(&opencode_id.to_string()));

            let native_models = build_byop_models_by_feature(ctx);
            let native_agent_mode_ids = choice_ids(&native_models.agent_mode);
            assert_eq!(
                native_agent_mode_ids.contains(&opencode_id.to_string()),
                cfg!(all(target_os = "macos", target_arch = "x86_64"))
            );
        });
    });
}

#[test]
fn smoke_lookup_byop_resolves_provider_and_model_without_api_key() {
    App::test((), |mut app| async move {
        init_byop_test_app(&mut app);

        let provider_id = "provider-lookup-1";
        app.update(|ctx| {
            AISettings::handle(ctx).update(ctx, |settings, ctx| {
                let _ = settings
                    .agent_providers
                    .set_value(vec![sample_provider(provider_id)], ctx);
            });
        });

        let encoded = llm_id::encode(provider_id, "llama3.2");
        app.read(|ctx| {
            let (provider, api_key, model_id) =
                lookup_byop(ctx, &encoded).expect("lookup_byop should resolve configured model");
            assert_eq!(provider.id, provider_id);
            assert_eq!(model_id, "llama3.2");
            assert!(api_key.is_empty(), "Ollama path allows empty API key");
        });
    });
}

#[test]
fn opencode_lookup_does_not_require_http_configuration_or_api_key() {
    App::test((), |mut app| async move {
        init_byop_test_app(&mut app);

        let provider_id = "opencode-lookup";
        let mut provider = sample_provider(provider_id);
        provider.kind = AgentProviderKind::OpenCodeCli;
        provider.base_url.clear();
        provider.cli_executable = Some(std::path::PathBuf::from("/usr/local/bin/opencode"));
        provider.models = vec![AgentProviderModel::from_id("openai/gpt-5".to_owned())];
        app.update(|ctx| {
            AISettings::handle(ctx).update(ctx, |settings, ctx| {
                let _ = settings.agent_providers.set_value(vec![provider], ctx);
            });
        });

        let encoded = llm_id::encode(provider_id, "openai/gpt-5");
        app.read(|ctx| {
            let (provider, api_key, model_id) =
                lookup_byop(ctx, &encoded).expect("OpenCode model should be resolvable");
            assert_eq!(provider.kind, AgentProviderKind::OpenCodeCli);
            assert_eq!(
                provider.cli_executable.as_deref(),
                Some(std::path::Path::new("/usr/local/bin/opencode"))
            );
            assert!(provider.base_url.is_empty());
            assert!(api_key.is_empty());
            assert_eq!(model_id, "openai/gpt-5");
        });
    });
}

#[test]
fn smoke_lookup_byop_returns_none_for_unknown_id() {
    App::test((), |mut app| async move {
        init_byop_test_app(&mut app);

        app.read(|ctx| {
            assert!(lookup_byop(ctx, &LLMId::from("byop:missing:model")).is_none());
            assert!(lookup_byop(ctx, &LLMId::from("not-byop")).is_none());
        });
    });
}
