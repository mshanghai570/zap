# OpenCode CLI as a Configurable Provider

## Context

Zap supports user-configured AI providers that communicate through HTTP APIs. It has a helper for launching OpenCode child tasks, but that helper is not connected to an execution path and hard-codes the executable and prompt invocation. The OpenCode CLI should be usable as an Agent Mode provider without relying on an HTTP endpoint.

This design adds a configurable OpenCode CLI profile that can be selected from Zap's Agent Mode provider picker. OpenCode remains responsible for model access, authentication, and its own agent tools.

## Goals

- Allow users to configure an OpenCode CLI profile in Zap's AI provider settings.
- Make the configured profile selectable from the Agent Mode provider picker.
- Keep OpenCode out of coding, CLI-agent model, computer-use, title-generation, and other HTTP-only model paths.
- Run OpenCode through its CLI, without requiring a separately configured HTTP endpoint or storing OpenCode credentials in Zap.
- Limit the new provider configuration and picker entry to Intel macOS (`x86_64-apple-darwin`).
- Preserve the app's ability to run on the user's Intel MacBook while allowing future, separately scoped optimization and bug-fix work.

## Non-goals

- Reimplementing OpenCode's agent loop or adapting OpenCode tool calls to Zap's native tool protocol.
- Making OpenCode a model for every feature that currently shares the BYOP model list.
- Wiring OpenCode into local child-agent execution or implementing child-agent task lifecycle/status handling.
- Replacing OpenCode's own authentication, model configuration, or permission behavior.
- Supporting this new provider on Apple Silicon, Windows, or Linux.
- Undertaking an unbounded repository-wide refactor as part of this provider change.

## Configuration

Add an OpenCode CLI provider kind to the existing AI provider settings. A profile contains:

- A user-visible name.
- The executable path, defaulting to `opencode` on `PATH`.
- A model identifier.

The profile is local machine configuration because executable paths and OpenCode installation/auth state are local. It does not store API credentials. Keep the initial configuration narrow: do not accept free-form shell fragments; pass the executable and supported arguments as separate arguments.

The Agent Mode picker displays configured OpenCode profiles/models alongside existing choices. Other model pickers keep their existing HTTP-provider choices because their request paths expect native model responses or Zap tool calls. Existing non-OpenCode provider entries and existing OpenCode-related behavior on other platforms remain unchanged. A future, separately scoped design may connect this profile to local child-agent execution.

## Runtime behavior

- Selecting an OpenCode profile in the Agent Mode picker delegates the submitted task to `opencode run` in the active project directory.
- Zap surfaces the run's output and completion/failure to the caller. OpenCode owns its model interaction and tool execution; Zap does not represent those tools as native Zap tool calls.
- The executable, model, and prompt are passed without shell-fragment interpolation.
- A missing executable, invalid configuration, authentication failure, or non-zero process exit is surfaced as an actionable error. Zap must not silently fall back to an HTTP endpoint or a different provider.
- OpenCode CLI profiles are not used for title generation, one-shot AI requests, or any model picker whose runtime expects an HTTP API or Zap-managed tool calls.

## Intel Mac compatibility

Gate the new provider settings/picker integration to Intel macOS. Leave the existing app and unrelated features available according to their current platform support. Validation for this feature must include the `x86_64-apple-darwin` target; Apple Silicon, Windows, and Linux validation is not a requirement for the new provider.

## Validation

- Unit tests cover settings serialization/defaults, profile lookup, argument construction, and actionable launch-failure handling.
- Verify OpenCode profiles appear only in the Agent Mode picker and do not enter HTTP-only one-shot/title-generation paths.
- Run the repository's documented Rust checks, including an Intel macOS target check when the target toolchain is available.
- Manual validation on an Intel Mac confirms profile setup, picker selection, task execution in the selected project, and visible handling of missing CLI/auth failures.

## Follow-up optimization and bug-fix work

Treat optimization and bug fixing as ongoing, separately scoped work. Start with evidence-backed issues in the AI provider and CLI-agent paths, prioritizing behavior and performance on Intel macOS. Expand to other app areas in bounded batches, fixing confirmed issues and validating each batch rather than making speculative repository-wide refactors. Local child-agent execution for OpenCode is a separate follow-up design because the current ambient-agent spawner is disabled and the standalone Agent SDK rejects OpenCode.
