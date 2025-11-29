````markdown

## Phase 1 – Core Rust + CLI + Quota (MVP)

### Goal

Deliver a working **Rust CLI + daemon + HTTP server** that:

- Wraps the `gemini` CLI for basic text prompts.
- Tracks usage in a local SQLite DB.
- Exposes a minimal OpenAI-ish `/v1/chat/completions` HTTP endpoint.
- Provides a `genie up` long-running process with a basic TUI-like status view.

This phase does **not** include PDF/repo processing or prompt gallery.

---

## Milestones

- **M1:** Project scaffolding + core crates layout. ✅
- **M2:** Gemini process wrapper (`GeminiClient`). ✅
- **M3:** SQLite usage DB + quota logic. ✅
- **M4:** CLI commands `ask`, `json`, `quota status`. ✅
- **M5:** HTTP server with `/v1/chat/completions`. ✅
- **M6:** `genie up` daemon + minimal TUI status. ✅
- **M7:** Packaging (`cargo install`), basic docs & CI. ✅

---

## Task Breakdown

### 1. Repository & Crate Structure

**Goal:** Create a clean, modular Rust workspace for Genie.

- [x] **1.1** Create a new Rust workspace:
  - `Cargo.toml` (workspace).
  - `genie-core` library crate.
  - `genie-cli` binary crate.
- [x] **1.2** In `genie-core`, scaffold modules:
  - [x] `config` – load/save config from `~/.genie/config.*`.
  - [x] `gemini` – Gemini CLI wrapper.
  - [x] `quota` – usage tracking + quota enforcement.
  - [x] `server` – HTTP API (Axum or similar).
  - [x] `model` – shared structs (requests/responses).
- [x] **1.3** In `genie-cli`, scaffold:
  - [x] `main.rs` using `clap`/`structopt`/`clap_derive` for CLI args.
  - [x] Subcommands: `ask`, `json`, `quota`, `up`, `status`.
- [x] **1.4** Set up basic logging:
  - [x] Use `tracing` or `env_logger`.
  - [x] Configurable log levels via env/CLI.

#### Acceptance Criteria

- [x] `cargo build` succeeds on clean checkout.
- [x] Running `cargo run -p genie-cli -- --help` shows subcommands.
- [x] Workspace uses clear module layout and Rust 2021+ edition.

#### Auto-Testing Prompt (for AI agent)

```text
You are an automated code reviewer. Inspect the repository structure and ensure:
- A Rust workspace with at least two crates exists: `genie-core` (lib) and `genie-cli` (bin).
- `genie-cli` defines a CLI with subcommands: `ask`, `json`, `quota`, `up`, `status`.
- The `genie-core` crate contains modules for `config`, `gemini`, `quota`, `server`, and shared `model` structs.
- `cargo build` and `cargo run -p genie-cli -- --help` succeed without warnings or errors.

Respond with:
- A checklist of findings.
- Any critical issues blocking Phase 1.
````

---

### 2. Configuration System

**Goal:** A small config file under `~/.genie` controlling defaults.

* [x] **2.1** Define config struct in `genie-core::config`:

  * [x] `gemini.binary: String` (default `"gemini"`).
  * [x] `gemini.default_model: String` (default `"gemini-2.5-pro"`).
  * [x] `server.host: String` (default `"127.0.0.1"`).
  * [x] `server.port: u16` (default `11435`).
  * [x] `quota.per_minute: u32` (default `60`).
  * [x] `quota.per_day: u32` (default `1000`).
  * [x] `quota.reset_time: String` (e.g., `"00:00"` local).
* [x] **2.2** Implement config loading order:

  * [x] Defaults → config file (`~/.genie/config.toml`) → environment → CLI overrides.
* [x] **2.3** Implement `genie-cli` global flags:

  * [x] `--config <path>`
  * [x] `--model <model_name>`
  * [x] `--port <u16>`
* [x] **2.4** Add `genie-cli config` subcommand (optional in Phase 1):

  * [x] `genie config show` – display effective config.

#### Acceptance Criteria

* [x] When no config file exists, defaults are used.
* [x] If config file exists, new values override defaults.
* [x] CLI flags override config values.
* [x] `genie-cli` can show effective config (either via `config show` or `--debug-config` option).

#### Auto-Testing Prompt

```text
You are an automated tester. Run the following checks:
1. Delete any existing config and run `genie config show` (or equivalent).
   - Verify default values match the spec: default model `gemini-2.5-pro`, host `127.0.0.1`, port `11435`, per_minute `60`, per_day `1000`.
2. Create a config file at `~/.genie/config.toml` and set `server.port = 9999`.
   - Run the CLI and verify it uses port 9999 by printing or logging the effective config.
3. Run `genie --port 7777 config show` and verify that port `7777` overrides the config file.

Return a JSON summary of:
- Steps executed.
- Expected vs actual values.
- Pass/fail for each step.
```

---

### 3. Gemini CLI Wrapper (`GeminiClient`)

**Goal:** Provide a robust async API for calling the `gemini` CLI.

* [x] **3.1** Implement `GeminiClient` struct in `genie-core::gemini`:

  * [x] Holds binary path, default model, and optional global system prompt.
* [x] **3.2** Implement a `GeminiRequest` struct:

  * [x] Fields: `model`, `prompt: String`, optional `system_prompt`, optional `temperature`, optional `max_tokens`.
* [x] **3.3** Implement `GeminiResponse` struct:

  * [x] Fields: `raw_output: String`, `text: String` (if applicable), optional `json: serde_json::Value`.
* [x] **3.4** Implement async function:

  * [x] `async fn call_text(&self, req: &GeminiRequest) -> Result<GeminiResponse, GeminiError>`
* [x] **3.5** Use `tokio::process::Command`:

  * [x] `stdin` piped.
  * [x] `stdout` piped.
  * [x] `stderr` piped.
* [x] **3.6** Support `--output-format json`:

  * [x] Parse stdout into JSON if requested.
  * [x] Handle `--model` and `-p` or stdin piping consistently.
* [x] **3.7** Error handling:

  * [x] Distinguish between:

    * CLI not found.
    * Authentication errors.
    * Quota / rate-limit errors.
    * Other process failures.
  * [x] Map errors into `GeminiError` enum.

#### Acceptance Criteria

* [x] A unit or integration test can mock/spawn a dummy `gemini` command that echoes input and verify `GeminiClient` handles stdout/stderr correctly.
* [x] If the `gemini` binary is missing, `GeminiClient` returns a clear `GeminiError::BinaryNotFound`.
* [x] If the CLI returns non-zero exit code, `GeminiClient` captures stderr and propagates an error.
* [x] For valid JSON output, `GeminiResponse.json` is populated.

#### Auto-Testing Prompt

```text
You are an automated test agent verifying the GeminiClient in `genie-core::gemini`.

1. Replace the real `gemini` binary in PATH with a test script that:
   - Prints a fixed JSON payload to stdout.
   - Exits with code 0.

2. Call `GeminiClient::call_text` with a simple prompt.
   - Verify:
     - `GeminiResponse.text` contains expected data.
     - `GeminiResponse.json` parses correctly.

3. Replace the test script to exit with code 1 and print an error to stderr.
   - Verify:
     - `GeminiClient::call_text` returns `Err(GeminiError::ProcessFailed { ... })`.
     - The error contains the stderr message.

Write a structured JSON report listing:
- Observed stdout and stderr.
- Parsed JSON results.
- Error variants matched.
- Pass/fail for each scenario.
```

---

### 4. Usage DB & Quota Logic

**Goal:** Track all Genie → Gemini calls and enforce configurable quotas.

* [x] **4.1** Add SQLite dependency (e.g., `sqlx` with `sqlite` feature).

* [x] **4.2** Initialize DB in `~/.genie/usage.db` on first run.

* [x] **4.3** Create `usage_events` table:

  ```sql
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  timestamp TEXT NOT NULL,
  model TEXT NOT NULL,
  kind TEXT NOT NULL,
  prompt_chars INTEGER NOT NULL,
  response_chars INTEGER NOT NULL,
  approx_input_tokens INTEGER NOT NULL,
  approx_output_tokens INTEGER NOT NULL,
  success BOOLEAN NOT NULL,
  error_code TEXT
  ```

* [x] **4.4** Implement `UsageEvent` struct and insert function in `genie-core::quota`.

* [x] **4.5** Implement helper queries:

  * [x] `fn count_requests_last_minute(...) -> u32`
  * [x] `fn count_requests_today(...) -> u32`
  * [x] `fn tokens_today(...) -> (u32, u32)` (input/output).

* [x] **4.6** Approximate tokens:

  * [x] Use a simple formula: `approx_tokens = (chars / 4)`.

* [x] **4.7** Implement `QuotaConfig` struct.

* [x] **4.8** Implement `QuotaManager` with:

  * [x] `fn check_before_request(kind, model) -> Result<(), QuotaError>`
  * [x] `fn record_after_request(event: UsageEvent)`

* [x] **4.9** Quota enforcement:

  * [x] Fail with `QuotaError` if `per_minute` or `per_day` would be exceeded.
  * [x] Add `--ignore-quota` CLI flag to bypass enforcement if explicitly requested.

#### Acceptance Criteria

* [x] Every successful Gemini call creates a row in `usage_events`.
* [x] A helper test can insert synthetic events and verify `count_requests_last_minute` and `count_requests_today` logic.
* [x] When simulated usage reaches the configured `per_day`, subsequent calls return a clear quota error unless `--ignore-quota` is passed.
* [x] Quota logic is time-zone aware for daily reset (or at least uses local midnight consistently).

#### Auto-Testing Prompt

```text
You are an automated tester for Genie's quota system.

1. Initialize a fresh usage DB and set `per_minute = 3`, `per_day = 5`.
2. Insert 3 successful events with timestamps in the last 60 seconds.
   - Call `QuotaManager::check_before_request` and verify it returns a "minute quota exceeded" error.
3. Insert 5 successful events with timestamps since local midnight.
   - Call `QuotaManager::check_before_request` and verify it returns a "daily quota exceeded" error.
4. Repeat step 3 but simulate passing `--ignore-quota` and verify the call is allowed.

Return a JSON report:
- For each check, include: inputs, expected outcome, actual outcome.
- Boolean `all_passed`.
```

---

### 5. CLI Commands: `ask`, `json`, `quota`

**Goal:** Provide the MVP CLI surface.

#### 5.1 `genie ask "prompt"`

* [x] Parse positional argument or read from stdin if no prompt provided.
* [x] Build `GeminiRequest` with:

  * Model from config/flag.
  * Prompt + optional global system prompt.
* [x] Check quota, call `GeminiClient`, record usage event.
* [x] Print plain text to stdout (strip JSON if CLI returns JSON envelope).
* [x] Exit code:

  * 0 on success.
  * > 0 on errors (quota, Gemini, etc.).

#### 5.2 `genie json "prompt" [--schema path]`

* [x] Similar to `ask` but:

  * Ensures JSON output.
  * If `--schema` provided:

    * Load JSON Schema.
    * Validate response.
    * On invalid JSON, re-prompt once with error message appended to prompt.
* [x] Print pretty-printed JSON to stdout.

#### 5.3 `genie quota status`

* [x] Query quota manager for:

  * `requests_today`, `per_day`.
  * `requests_last_minute`, `per_minute`.
  * `tokens_today`.
* [x] Print human-readable summary.
* [x] Optionally support `--json` flag.

#### Acceptance Criteria

* [x] `genie ask "hello"` produces a non-empty response when `gemini` is available.
* [x] `genie ask` with stdin (`echo "hi" | genie ask`) works.
* [x] `genie json` prints valid JSON and uses schema if provided.
* [x] After several calls, `genie quota status` displays updated usage numbers.

#### Auto-Testing Prompt

```text
You are an automated tester executing CLI commands for Genie.

Assume a working `gemini` binary that returns deterministic responses.

1. Run: `echo "Test prompt" | genie ask`
   - Confirm:
     - Exit code is 0.
     - stdout is non-empty.
     - A `usage_events` row is created.

2. Run: `genie json "Return a JSON object {\"ok\": true}"`
   - Confirm:
     - Output is valid JSON.
     - JSON contains key `"ok": true`.

3. Run: `genie quota status`
   - Confirm:
     - Output mentions "Requests today" and "Requests last minute".
     - Numbers are consistent with the calls made in steps 1–2.

Return a JSON summary of:
- Commands executed.
- Their stdout/stderr.
- DB checks performed.
- Pass/fail.
```

---

### 6. HTTP Server & `/v1/chat/completions`

**Goal:** Run a small HTTP server exposing an OpenAI-style endpoint.

* [x] **6.1** In `genie-core::server`, initialize Axum (or similar) server:

  * [x] Bind to `host:port` from config.

* [x] **6.2** Define request/response models closely resembling OpenAI:

  ```rust
  struct ChatCompletionRequest {
      model: String,
      messages: Vec<ChatMessage>,
      max_tokens: Option<u32>,
      temperature: Option<f32>,
      stream: Option<bool>,
      // ignore other fields for now
  }

  struct ChatMessage {
      role: String,   // "system" | "user" | "assistant"
      content: String,
  }

  struct ChatCompletionResponse {
      id: String,
      object: String,
      created: i64,
      model: String,
      choices: Vec<Choice>,
  }

  struct Choice {
      index: u32,
      message: ChatMessage,
      finish_reason: String,
  }
  ```

* [x] **6.3** Implement `POST /v1/chat/completions`:

  * [x] Accept JSON request.
  * [x] Convert messages into a single prompt or appropriate format.
  * [x] Check quota.
  * [x] Call `GeminiClient`.
  * [x] Build response in the OpenAI-compatible shape.

* [x] **6.4** Add to daemon startup (see next section).

#### Acceptance Criteria

* [x] Starting the server (via `genie up` or a dedicated command) binds to the configured port and logs a startup message.
* [x] Sending a minimal chat completion request returns a valid JSON response.
* [x] Incorrect payloads produce a 4xx error with helpful message.
* [x] Quota is enforced on HTTP calls as well.

#### Auto-Testing Prompt

```text
You are an automated API tester for Genie's HTTP server.

1. Start the Genie server (daemon mode) and wait until it is ready on `http://127.0.0.1:11435`.
2. Send a POST request to `/v1/chat/completions` with JSON:
   {
     "model": "gemini-2.5-pro",
     "messages": [
       {"role": "user", "content": "Say hello in JSON as {\"greet\": \"hi\"}"}
     ]
   }
3. Verify:
   - HTTP status is 200.
   - Response JSON has `choices[0].message.content` containing text.
   - A `usage_events` entry is created.

4. Send an invalid payload (e.g., missing `messages`).
   - Verify server returns a 4xx error with a JSON error message.

Provide a JSON report listing:
- Request bodies.
- Response codes and bodies.
- DB checks.
- Pass/fail for each scenario.
```

---

### 7. Daemon Mode: `genie up` + Minimal TUI

**Goal:** Long-running process with a basic terminal UI similar in spirit to Tilt.

* [x] **7.1** Add `genie up` subcommand:

  * [x] Starts:

    * HTTP server.
    * TUI loop.
  * [x] Handles graceful shutdown on `Ctrl+C` or `q`.
* [x] **7.2** Implement TUI using `ratatui` or similar:

  * [x] Layout:

    * Status bar with:

      * Requests today / per_day.
      * Requests last minute / per_minute.
    * Log list: recent requests (time, kind, success).
  * [x] Keybindings:

    * `q` – exit.
    * `space` – toggle between compact and expanded view.
* [x] **7.3** Integrate with `QuotaManager`:

  * [x] TUI periodically refreshes metrics (e.g., every 1–2 seconds).
* [x] **7.4** Ensure TUI runs in an async-friendly loop without blocking HTTP server.

#### Acceptance Criteria

* [x] Running `genie up` starts the server and shows a live TUI.
* [x] Making HTTP or CLI requests while `genie up` runs updates the TUI usage numbers and log.
* [x] Pressing `q` cleanly shuts down the server and UI.
* [x] Pressing `space` switches between at least two views (e.g., minimal vs detailed).

#### Auto-Testing Prompt

```text
You are an automated tester verifying daemonic behavior and TUI integration.

Because automated testing of TUI is hard, focus on side effects:

1. Start `genie up` in a background process.
   - Wait until logs show the server is listening on the configured port.

2. While `genie up` is running:
   - Send 2 HTTP calls to `/v1/chat/completions`.
   - After a brief delay, run `genie quota status` in another process.

3. Confirm:
   - The quota status reflects the 2 HTTP calls made.
   - The `genie up` process is still alive.

4. Send a termination signal or simulate the user pressing `q`.
   - Confirm the process exits cleanly.

Return a JSON report summarizing:
- Server startup detection.
- Requests made and logged.
- Shutdown behavior.
```

---

### 8. Packaging, CI & Basic Docs

**Goal:** Make Phase 1 installable and minimally documented.

* [x] **8.1** Add `cargo install` support:

  * [x] Ensure `genie-cli` is the default binary crate.
* [x] **8.2** Add GitHub Actions workflow:

  * [x] On push/PR, run:

    * `cargo fmt --check`
    * `cargo clippy -- -D warnings`
    * `cargo test --all`
* [x] **8.3** Add basic README sections:

  * [x] How to install (`cargo install`).
  * [x] How to configure Gemini CLI and run `genie ask`.
  * [x] Warning that Gemini CLI must be installed and authenticated.
* [x] **8.4** Add `--version` and `--help` tests in CI (optional).

#### Acceptance Criteria

* [x] Running `cargo install --path .` installs `genie` binary successfully.
* [x] GitHub Actions passes on a fresh clone.
* [x] README clearly explains:

  * Prerequisites (Gemini CLI).
  * Basic usage (`ask`, `json`, HTTP endpoint, `up`).
* [x] `genie --version` prints a semantic version.

#### Auto-Testing Prompt

```text
You are an automated CI validation agent.

1. Clone the repository in a clean environment with Rust stable.
2. Run:
   - `cargo fmt --check`
   - `cargo clippy -- -D warnings`
   - `cargo test --all`
3. Run `cargo install --path .` and then `genie --version`.

Return a JSON object:
- `fmt_passed`: bool
- `clippy_passed`: bool
- `tests_passed`: bool
- `install_succeeded`: bool
- `version_output`: string
- `problems`: list of string
```

````

---
