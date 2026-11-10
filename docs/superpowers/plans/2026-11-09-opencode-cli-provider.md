# OpenCode CLI Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a configurable OpenCode CLI provider to Zap's Agent Mode picker on Intel macOS, running requests through `opencode run` without an HTTP endpoint.

**Architecture:** Extend the existing local `AgentProvider` settings with an OpenCode CLI kind and executable path, while retaining the existing model list for manually configured model IDs. Show these models only for Agent Mode, invoke OpenCode through the workspace command wrapper, and adapt its output to the existing agent response stream. Keep OpenCode child-agent lifecycle work out of this implementation.

**Tech Stack:** Rust, Cargo workspace, WarpUI settings views, serde/TOML, Fluent localization, `command::async::Command`, existing BYOP response-stream event types.

**Spec:** `docs/superpowers/specs/2026-11-09-opencode-cli-provider-design.md`

## Global Constraints

- Limit the new provider configuration and picker entry to Intel macOS (`x86_64-apple-darwin`).
- Keep OpenCode out of coding, CLI-agent model, computer-use, title-generation, and other HTTP-only model paths.
- Do not require a separately configured HTTP endpoint or store OpenCode credentials in Zap.
- Pass the executable, model, and prompt as separate process arguments; do not interpolate shell fragments.
- Do not route OpenCode through the standalone Agent SDK driver or implement local child-agent lifecycle/status handling in this change.
- Preserve existing non-OpenCode provider behavior and existing OpenCode-related behavior on other platforms.

---

## File map

- `app/src/settings/ai.rs` — Provider-kind enum, optional OpenCode executable path, and defaults.
- `app/src/settings/ai_tests.rs` — Backward compatibility and settings serialization tests.
- `app/src/ai/agent_providers/mod.rs` — Build Agent Mode choices from endpoint and CLI profiles; keep other feature choices HTTP-only.
- `app/src/ai/agent_providers/mod_test.rs` — Model-picker visibility and provider lookup tests.
- `app/src/settings_view/agent_providers_widget.rs` — Provider-kind selection, CLI path field, and kind-specific form rendering.
- `app/src/settings_view/ai_page.rs` — Settings actions and persistence for provider kind/path.
- `app/i18n/{en,zh-CN,ja,ru}/warp.ftl` — Localized labels and help text for the new provider controls.
- `app/src/ai/agent_providers/opencode_cli.rs` — Process invocation and CLI-specific error mapping.
- `app/src/ai/agent_providers/opencode_cli_tests.rs` — Argument handling and process failure tests.
- `app/src/ai/agent_providers/chat_stream.rs` — Construct Zap response events for the completed OpenCode run, reusing existing event builders.
- `app/src/ai/blocklist/controller/response_stream.rs` — Route OpenCode profiles to the CLI-backed response generator on initial requests and retries; keep title generation on HTTP providers.
- `app/src/ai/blocklist/controller/response_stream_tests.rs` — Provider-kind dispatch and response-stream regression tests.
- `app/src/ai/agent_providers/oneshot.rs` — Reject OpenCode profiles from HTTP-only one-shot paths.
- `app/src/ai/blocklist/agent_view/agent_input_footer/reasoning_depth_selector.rs` — Do not offer API reasoning controls for OpenCode.

## Task 1: Add OpenCode settings and isolate its model choices

**Files:**
- Modify: `app/src/settings/ai.rs`
- Test: `app/src/settings/ai_tests.rs`
- Modify: `app/src/ai/agent_providers/mod.rs`
- Test: `app/src/ai/agent_providers/mod_test.rs`

**Interfaces:**
- Produces `AgentProviderKind::OpenCodeCli`.
- Produces `AgentProvider::cli_executable: Option<PathBuf>`; `None` means resolve `opencode` from `PATH`.
- Keeps `AgentProvider::models` as the user-entered OpenCode model list.
- The target-aware model builder can include OpenCode only in `ModelsByFeature.agent_mode`; the public picker remains HTTP-only until Task 4 wires CLI dispatch, then exposes OpenCode only on Intel macOS. `coding`, `cli_agent`, and `computer_use` remain HTTP-only.

- [ ] **Step 1: Add failing settings compatibility tests**

In `app/src/settings/ai_tests.rs`, add tests that deserialize an old provider TOML entry without a kind or executable path, and round-trip an OpenCode CLI provider with no `base_url`:

```rust
#[test]
fn opencode_cli_provider_settings_round_trip() {
    let mut provider = AgentProvider::new_empty();
    provider.kind = AgentProviderKind::OpenCodeCli;
    provider.cli_executable = Some(PathBuf::from("/opt/homebrew/bin/opencode"));
    provider.models = vec![AgentProviderModel::from_id("openai/gpt-5".to_owned())];

    let serialized = toml::to_string(&provider).expect("provider should serialize");
    let restored: AgentProvider =
        toml::from_str(&serialized).expect("provider should deserialize");

    assert_eq!(restored, provider);
    assert!(restored.base_url.is_empty());
}
```

Add a second test asserting that an old TOML provider without `cli_executable` defaults it to `None` and retains the existing HTTP provider kind.

- [ ] **Step 2: Run the settings tests and verify the new test fails to compile**

Run: `cargo test -p warp --lib opencode_cli_provider_settings_round_trip`

Expected: FAIL because the provider kind and executable-path field do not exist yet.

- [ ] **Step 3: Add the provider kind and optional executable path**

In `app/src/settings/ai.rs`, add the serializable, iterable `OpenCodeCli` kind and its display name, add `cli_executable: Option<PathBuf>` with serde default/omit-when-`None`, and initialize it to `None` in `AgentProvider::new_empty`. Leave `AgentProviderApiType` unchanged; OpenCode is a process provider, not an additional HTTP protocol.

Update every `AgentProvider` struct literal in `app/src/settings/ai_tests.rs` and `app/src/ai/agent_providers/mod_test.rs` with `cli_executable: None`.

- [ ] **Step 4: Add model-choice isolation tests**

In `app/src/ai/agent_providers/mod_test.rs`, configure one HTTP provider and one OpenCode provider with an empty base URL and a model each. Assert that Agent Mode choices contain both models while coding, CLI-agent, and computer-use choices contain only the HTTP model. Also assert that OpenCode is not exposed as a selectable provider on non-Intel targets.

- [ ] **Step 5: Separate Agent Mode choices from HTTP-only choices**

In `app/src/ai/agent_providers/mod.rs`, accept a blank base URL only for `OpenCodeCli`, continue requiring at least one non-empty model ID, and build separate `AvailableLLMs` sets. The target-aware helper's Agent Mode set includes both kinds when requested; its other sets include only HTTP providers. Keep `build_byop_models_by_feature` HTTP-only in this task so users cannot select OpenCode before CLI request dispatch exists. Preserve the existing unavailable placeholder when a feature has no valid choices.

- [ ] **Step 6: Run provider-model tests**

Run: `cargo test -p warp --lib opencode_cli`

Expected: PASS for settings round-trip, legacy defaults, target-aware Agent Mode model construction, and HTTP-only picker isolation. The public picker must not expose OpenCode until Task 4.

- [ ] **Step 7: Commit the settings/model changes**

```bash
git add app/src/settings/ai.rs app/src/settings/ai_tests.rs app/src/ai/agent_providers/mod.rs app/src/ai/agent_providers/mod_test.rs
git commit -m "feat: add OpenCode CLI provider settings" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

## Task 2: Add Intel-Mac provider settings UI

**Files:**
- Modify: `app/src/settings_view/agent_providers_widget.rs`
- Modify: `app/src/settings_view/ai_page.rs`
- Modify: `app/i18n/en/warp.ftl`
- Modify: `app/i18n/zh-CN/warp.ftl`
- Modify: `app/i18n/ja/warp.ftl`
- Modify: `app/i18n/ru/warp.ftl`

**Interfaces:**
- Provider cards can select `OpenAiCompatible` or `OpenCodeCli`.
- OpenCode CLI cards edit the executable path and model list; they do not render base URL, API key, API protocol, extra HTTP headers, or API model-fetch controls.
- The save action persists provider kind, executable path, display name, and model rows through `AISettings.agent_providers`.

- [ ] **Step 1: Add provider-kind and executable-path draft state**

Extend `ProviderRow` and `ProviderDraftEditors` in `agent_providers_widget.rs` with an executable-path editor and a provider-kind selector state. Add a draft value for provider kind and executable path to the save action created from the editor handles.

- [ ] **Step 2: Add settings actions for kind and executable path**

In `ai_page.rs`, extend `AISettingsPageAction` and its `handle_action` match for setting `AgentProviderKind` and saving `cli_executable`. Preserve the existing name, model, and HTTP-provider save behavior.

- [ ] **Step 3: Render kind-specific provider card fields**

In `agent_providers_widget.rs`, render the kind selector for Intel macOS. For OpenCode CLI, show the localized executable-path field with `opencode` as the default placeholder, retain the manual model editor, and hide endpoint/API-key/header/protocol/fetch controls. For HTTP providers, keep the current fields and controls unchanged.

- [ ] **Step 4: Add localized labels and help text**

Add matching Fluent keys in all four locale catalogs for the provider kind, OpenCode CLI choice, executable path, and local-auth/model help text. Preserve existing key names and translations for HTTP-provider controls.

- [ ] **Step 5: Build the settings UI**

Run: `cargo check -p warp`

Expected: PASS; Fluent keys resolve and provider-card action types compile.

- [ ] **Step 6: Commit the settings UI**

```bash
git add app/src/settings_view/agent_providers_widget.rs app/src/settings_view/ai_page.rs app/i18n/en/warp.ftl app/i18n/zh-CN/warp.ftl app/i18n/ja/warp.ftl app/i18n/ru/warp.ftl
git commit -m "feat: configure OpenCode CLI provider in settings" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

## Task 3: Implement the OpenCode CLI process adapter

**Files:**
- Create: `app/src/ai/agent_providers/opencode_cli.rs`
- Create: `app/src/ai/agent_providers/opencode_cli_tests.rs`
- Modify: `app/src/ai/agent_providers/mod.rs`

**Interfaces:**
- `OpenCodeRunConfig` contains `executable: PathBuf`, `model_id: String`, `working_directory: PathBuf`, and `prompt: String`.
- `run_opencode(config, cancellation_rx) -> Result<String, OpenCodeCliError>` invokes the configured executable with discrete arguments, returns stdout on success, and returns a descriptive error including stderr on non-zero exit. Resolve `cli_executable: None` to `PathBuf::from("opencode")` before constructing `OpenCodeRunConfig`.
- `opencode_args(model_id, prompt) -> Vec<OsString>` returns `["run", "--model", model_id, prompt]`; the runner passes each value to `Command::arg`.
- The module uses `command::async::Command`; it must not invoke `std::process::Command` directly or construct a shell command string.

- [ ] **Step 1: Add failing argument and failure tests**

In `opencode_cli_tests.rs`, test that `opencode_args` returns `run`, `--model`, the exact model ID, and the complete prompt as four distinct arguments, including when the model ID or prompt contains spaces, quotes, and shell metacharacters. Add tests for a missing executable and a non-zero child exit with stderr.

- [ ] **Step 2: Run the process-adapter tests and verify they fail**

Run: `cargo test -p warp --lib opencode_cli`

Expected: FAIL because the OpenCode process adapter has not been implemented.

- [ ] **Step 3: Add the process config, command builder, and error type**

Implement `OpenCodeRunConfig` and `OpenCodeCliError`. Build the invocation as the executable plus `run`, the model option, and the prompt as separate `Command::arg` values; set `current_dir` to the active project directory and capture stdout/stderr. Do not use a shell, endpoint, or Zap-managed API secret.

- [ ] **Step 4: Handle cancellation, spawn errors, and non-zero exits**

Race process completion against the response cancellation signal. On cancellation, terminate the spawned process and return the established cancellation result; on spawn or non-zero exit, preserve the underlying error and surface stderr where available.

- [ ] **Step 5: Run the process-adapter tests**

Run: `cargo test -p warp --lib opencode_cli`

Expected: PASS for argument boundaries, missing executable, non-zero status, and cancellation behavior.

- [ ] **Step 6: Commit the process adapter**

```bash
git add app/src/ai/agent_providers/opencode_cli.rs app/src/ai/agent_providers/opencode_cli_tests.rs app/src/ai/agent_providers/mod.rs
git commit -m "feat: run OpenCode through its CLI" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

## Task 4: Route Agent Mode requests through OpenCode

**Files:**
- Modify: `app/src/ai/agent_providers/chat_stream.rs`
- Modify: `app/src/ai/agent_providers/mod.rs`
- Modify: `app/src/ai/blocklist/controller/response_stream.rs`
- Modify: `app/src/ai/agent_providers/oneshot.rs`
- Modify: `app/src/ai/blocklist/agent_view/agent_input_footer/reasoning_depth_selector.rs`
- Test: `app/src/ai/agent_providers/mod_test.rs`
- Test: `app/src/ai/blocklist/controller/response_stream_tests.rs`

**Interfaces:**
- `generate_opencode_output` adapts one OpenCode CLI result to the existing `api::ResponseStream`, using the same task initialization, input persistence, assistant-output, completion, and error conventions as `generate_byop_output`.
- `ByopDispatch` carries provider kind and executable path so `ResponseStream::new` and retry dispatch to OpenCode or HTTP explicitly.
- Title generation, one-shot requests, and API reasoning controls accept only `OpenAiCompatible` providers.

- [ ] **Step 1: Add a failing response-dispatch test**

Create `response_stream_tests.rs`, include it from `response_stream.rs` under `#[cfg(test)]`, and test the pure provider-kind dispatch decision: `OpenCodeCli` selects the CLI-backed generator and `OpenAiCompatible` selects `generate_byop_output`. In `mod_test.rs`, assert that an OpenCode profile cannot resolve as an `OneshotConfig`.

- [ ] **Step 2: Implement OpenCode response-event conversion**

In `chat_stream.rs`, add `generate_opencode_output` alongside the existing BYOP generator. Emit `StreamInit`, create the task when needed, persist the current user input, append the returned assistant text, then emit the existing successful stream-finished event. Reuse existing event constructors rather than duplicating their protobuf structure. Convert process failures to the existing user-visible `AIApiError` stream form.

- [ ] **Step 3: Route initial requests and retries by provider kind**

In `response_stream.rs`, carry `AgentProviderKind` and the configured executable in `ByopDispatch`. In both `ResponseStream::new` and `retry`, call `generate_opencode_output` only for `OpenCodeCli`; retain `generate_byop_output` for `OpenAiCompatible`. Filter OpenCode from title generation. Only after both dispatch paths are wired, update `build_byop_models_by_feature` in `agent_providers/mod.rs` to expose OpenCode models on Intel macOS; keep the other feature lists HTTP-only. Update `mod_test.rs` to assert that the public picker includes OpenCode on Intel macOS, excludes it on other targets, and keeps it out of coding, CLI-agent, and computer-use choices.

- [ ] **Step 4: Guard HTTP-only helpers**

In `oneshot.rs`, return no `OneshotConfig` for `OpenCodeCli`. In `reasoning_depth_selector.rs`, do not offer API reasoning controls for OpenCode. Keep the provider-list feature filtering from Task 1 as the primary guard for coding, CLI-agent, and computer-use pickers.

- [ ] **Step 5: Run focused provider and response-stream tests**

Run: `cargo test -p warp --lib opencode_cli`

Expected: PASS; OpenCode is dispatched only through the process runner, HTTP providers remain on the existing stream path, and HTTP-only helper paths reject OpenCode.

- [ ] **Step 6: Commit Agent Mode routing**

```bash
git add app/src/ai/agent_providers/chat_stream.rs app/src/ai/blocklist/controller/response_stream.rs app/src/ai/agent_providers/oneshot.rs app/src/ai/blocklist/agent_view/agent_input_footer/reasoning_depth_selector.rs app/src/ai/agent_providers/mod_test.rs
git commit -m "feat: route Agent Mode through OpenCode CLI" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

## Task 5: Verify the Intel Mac behavior end to end

**Files:**
- Test: `app/src/settings/ai_tests.rs`
- Test: `app/src/ai/agent_providers/mod_test.rs`
- Test: `app/src/ai/agent_providers/opencode_cli_tests.rs`
- Manual: Zap on Intel macOS with OpenCode installed and authenticated.

**Interfaces:**
- No new production interfaces. This task verifies the settings, picker, process, and response-stream contracts from Tasks 1–4 together.

- [ ] **Step 1: Run formatting validation**

Run: `cargo fmt --all -- --check`

Expected: PASS with no formatting changes required.

- [ ] **Step 2: Run focused tests**

Run: `cargo test -p warp --lib opencode_cli`

Expected: PASS for profile persistence, model-list isolation, process invocation, error handling, and response dispatch.

- [ ] **Step 3: Check the Intel macOS target**

Run: `cargo check --target x86_64-apple-darwin -p warp`

Expected: PASS. If Rust reports the target is not installed, run `rustup target add x86_64-apple-darwin`, then rerun the check.

- [ ] **Step 4: Manually validate on the Intel Mac**

Configure an OpenCode CLI provider with a display name, executable path (or default `opencode`), and model ID. Confirm it appears in the Agent Mode picker only; submit a task in a project and verify OpenCode runs there and its response appears in Zap. Then test a missing executable and an authentication failure; both must be visibly reported without falling back to HTTP. Confirm existing HTTP providers still work.

- [ ] **Step 5: Commit verification-driven fixes**

If validation required fixes, stage only the feature files changed while fixing them and commit:

```bash
git add app/src/settings/ai.rs app/src/settings/ai_tests.rs app/src/settings_view/agent_providers_widget.rs app/src/settings_view/ai_page.rs app/src/ai/agent_providers/mod.rs app/src/ai/agent_providers/mod_test.rs app/src/ai/agent_providers/opencode_cli.rs app/src/ai/agent_providers/opencode_cli_tests.rs app/src/ai/agent_providers/chat_stream.rs app/src/ai/blocklist/controller/response_stream.rs app/src/ai/agent_providers/oneshot.rs app/src/ai/blocklist/agent_view/agent_input_footer/reasoning_depth_selector.rs app/i18n/en/warp.ftl app/i18n/zh-CN/warp.ftl app/i18n/ja/warp.ftl app/i18n/ru/warp.ftl
git commit -m "fix: address OpenCode provider validation findings" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```
