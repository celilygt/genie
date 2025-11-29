# Genie – Product Requirements Document (PRD)

## 1. Overview

**Product name:** Genie
**Type:** Local-first Rust application + CLI that wraps the official `gemini` CLI
**Primary goals:**

* Provide a **local “Gemini-as-a-service”** backend using the Gemini CLI (no direct API keys).
* Offer **strong quota tracking** and usage visibility (requests/min, requests/day, approx tokens).
* Provide **power tools** for:

  * Batch PDF/book summarization with chapter detection.
  * Codebase / repo summarization.
  * Sed-like map/transform pipelines on text.
  * Prompt gallery / templates.
* Expose an **OpenAI-style HTTP API** to plug into LangChain and other frameworks.
* Showcase **Rust proficiency** (async, process orchestration, Tauri, low-level PDF work).

**Non-goals (v1):**

* No direct calls to Google Gemini HTTP APIs; Genie **must** talk only via the official `gemini` CLI.
* No Google-native/Vertex-only features (Vertex RAG etc.).
* No multi-tenant / multi-user server. Genie is **single-user / local**.

---

## 2. Target Users & Core Use Cases

### 2.1 Primary user

* Single developer / power user who:

  * Has Gemini CLI installed and authenticated.
  * Uses macOS or Linux.
  * Is comfortable with CLI and wants a GUI for some workflows.
  * Wants to build AI apps (e.g., with LangChain) without managing Gemini API keys.

### 2.2 Key use cases

1. **Local Gemini backend for scripts/tools**

   * Call Genie via CLI or HTTP instead of `gemini` directly.
   * Use OpenAI-compatible HTTP API to plug into LangChain or other frameworks.

2. **Quota & usage dashboard**

   * Track actual calls through Genie.
   * See daily/minute usage and approximate token use.
   * Always know how close you are to free-tier limits.

3. **Batch PDF / book summarization**

   * Drag-and-drop or CLI: summarize PDFs chapter-by-chapter.
   * Auto chapter detection using a heuristic + LLM hybrid.
   * Output JSON and/or Markdown, structured and machine-consumable.

4. **Codebase / repo summarization (“repo vibe”)**

   * Point Genie at a git repo.
   * Get architecture summaries, module overviews, and maybe per-folder summaries.
   * Use batching to stay under context limits but keep results coherent.

5. **Sed-like text map/transform**

   * Pipe text through prompts:

     * “Make each line a bullet summary”
     * “Rewrite each block more formally”
   * Use Genie easily in shell scripts and pipelines.

6. **Prompt gallery / templates**

   * Save common prompts with parameters (files, variables).
   * Run them from CLI and GUI.
   * Use them as “tools” when calling Genie from other apps.

7. **(Later) RAG / embeddings**

   * Use Genie to build small local RAG-style apps:

     * Store embeddings into a vector store (e.g., Chroma or another local store).
     * Query via Genie’s HTTP API.

---

## 3. Product Scope & Features

### 3.1 Core binaries & components

**Binaries / crates:**

1. `genie-core` (Rust library crate)

   * Business logic: process orchestration, quota tracking, PDF and repo analysis, prompt evaluation.

2. `genie-cli` (Rust binary)

   * CLI interface, TUI/daemon mode (Tilt-like `genie up`).
   * Talks to `genie-core`.

3. `genie-ui` (Tauri app)

   * Desktop UI with:

     * Main window (workspaces).
     * macOS menu bar/tray icon for quick quota + settings.
   * Talks to Genie backend via IPC/HTTP or direct Rust bindings.

4. `gemini` (external dependency)

   * Installed separately (npm/brew) and accessible on `$PATH`.

---

### 3.2 CLI: Commands & Behaviors

#### 3.2.1 Basic commands

1. `genie ask "prompt"`

   * **Description:** Simple one-shot call to Gemini via CLI.
   * **Behavior:**

     * Builds a request with default model & system prompt.
     * Invokes `gemini` in non-interactive mode with `--output-format json` where possible.
     * Prints plain text answer to stdout by default.
     * Logs request in usage DB.

2. `genie json "prompt" [--schema path]`

   * **Description:** Prompt that **must** return JSON.
   * **Behavior:**

     * Optionally loads a JSON Schema from file.
     * Calls `gemini` with instructions and/or response schema.
     * Validates output against schema.
     * On invalid JSON: re-prompt once with error feedback; if still invalid, returns error.
     * Output is JSON to stdout.

3. `genie map --prompt "..."`

   * **Usage:** `cat input.txt | genie map --prompt "Summarize each line"`
   * **Behavior:**

     * Reads stdin as text; splits by line or configurable separator.
     * For each chunk, calls Gemini using a template (map operation).
     * Concurrency limit configurable (e.g., `--concurrency 4`).
     * Writes outputs line-by-line.

4. `genie transform --prompt "..."`

   * Similar to `map`, but treats entire stdin as one block.

#### 3.2.2 Daemon / Tilt-like mode

1. `genie up`

   * **Description:** Start Genie as a long-running background process with a TUI.
   * **Behavior:**

     * Starts HTTP server (OpenAI-compatible + internal API) listening on configured port.
     * Displays a TUI in current terminal:

       * Top/side status bar: current quotas, recent calls, active jobs.
       * Log of recent requests.
     * Keybindings:

       * `space`: toggle expanded view / minimal view (Tilt-like).
       * `q`: quit the process.
     * Continues to serve HTTP API while TUI is active.
     * Optionally, can spawn/notify Tauri GUI.

2. `genie status`

   * If daemon is running, query its status and print quotas, uptime, etc.

3. `genie stop`

   * Send a signal to stop the running daemon.

Implementation: Use something like `ratatui` or `crossterm` for terminal UI.

#### 3.2.3 Quota & usage

1. `genie quota status`

   * Prints:

     * Requests today vs configured daily limit.
     * Requests last minute vs per-minute limit.
     * Approx input/output tokens.
     * Last error (if any).

2. `genie quota log [--last N]`

   * Shows recent calls with:

     * Time, model, type (ask/json/pdf/repo...).
     * Success/failure, error code if any.
     * Approx tokens.

3. `genie quota config`

   * Allows setting:

     * `per_minute`, `per_day`.
     * Local “reset at HH:MM” time.
   * Writes to config file.

Quota enforcement:

* Before each Gemini call:

  * Check local DB for recent events.
  * If exceeding limit:

    * Fail with clear CLI error by default.
    * Allow `--ignore-quota` override.

---

### 3.3 PDF / book processor

#### 3.3.1 CLI interface

1. `genie summarize-pdf <file> [options]`

   * Summarizes entire PDF (no chapters) into JSON/Markdown.

2. `genie summarize-book <file> [options]`

   * Handles chapter detection + per-chapter summaries.

Common flags:

* `--style concise|detailed|exam-notes` (text style hint).
* `--language <code>` for response language.
* `--out <path>` (output file).
* `--format json|markdown|both` (default: both).
* `--prompt-id <prompt_name>` or `--prompt-file <template>` for custom summarization prompt.
* `--max-chunk-tokens <n>` to tune batch size.

#### 3.3.2 Internal pipeline

For `summarize-book`:

1. **PDF extraction (Rust):**

   * Use `lopdf` (or similar) to access:

     * Page text.
     * Font sizes and styles (bold).
   * Build an in-memory representation:

     * Pages → blocks → (text, font, size, position).

2. **Heuristic candidate chapter detection:**

   * Compute font size histogram → infer “body text” size.
   * Find text blocks significantly larger and/or bold → candidates.
   * Apply regex to filter chapter-like titles:

     * e.g., `^Chapter \d+`, `^\d+\.\s+`, roman numerals, etc.

3. **Semantic chapter verification (Gemini call):**

   * Build a compact prompt with:

     * Extracted ToC (if present).
     * Candidate titles + page numbers.
   * Ask Gemini:

     * “Return JSON list of confirmed chapters with titles and start/end pages.”
   * Deserialize to struct `{ chapter_id, title, start_page, end_page }`.

4. **Per-chapter summarization (Map-Reduce style):**

   * For each chapter:

     * Extract plain text from `start_page..end_page`.
     * If chapter exceeds `max_chunk_tokens`, split into parts.
     * For each chunk:

       * Call summarization prompt (customizable via template).
     * Combine chunk summaries into a final chapter summary via another Gemini call.
   * Each chapter’s result must be valid JSON matching a Rust `ChapterSummary` struct.

5. **Global summary:**

   * Combine all chapter data and call Gemini once more to:

     * Create global summary.
     * Suggest reading roadmap.
   * Output aggregated JSON and Markdown.

6. **Output:**

   * JSON file:

     * Book-level fields (`title`, `author`, etc.).
     * Array of chapters with structure:
       `{ id, title, summary, key_points, important_terms, questions }`
   * Markdown file summarizing contents.

---

### 3.4 Code / repo summary features

#### 3.4.1 CLI commands

1. `genie repo-summary [path]`

   * Summarizes a codebase.

**Behavior:**

* Walk directory using `walkdir`.

* Respect `.gitignore` and ignore binaries.

* Group files by:

  * Directory.
  * Language (extension).

* Build text blocks like:

  ```text
  === file: src/main.rs ===
  <file contents or truncated content>
  ```

* For large repos:

  * Split into chunks (e.g. per-directory) under context limits.
  * For each chunk, call Gemini for:

    * Overview of files.
    * Main responsibilities / architecture.
  * Combine chunk-level summaries into top-level overview.

* Output:

  * JSON with per-directory summaries.
  * Markdown for human reading.

2. `genie repo-files [path] --pattern <glob>`

   * Helper for listing & selecting files to pass through other prompts (optional).

---

### 3.5 Prompt gallery / templates

#### 3.5.1 Storage format

* Location: `~/.genie/prompts/*.prompt.md`
* Format: Markdown with YAML frontmatter, e.g.:

  ```yaml
  name: "book-summarize"
  description: "Summarize a book chapter-by-chapter"
  model: "gemini-2.5-pro"
  input_variables:
    - name: "style"
      description: "Summary style"
      default: "concise"
    - name: "language"
      description: "Output language"
      default: "en"
    - name: "file"
      type: "file"
      description: "Input file path"
  ---
  You are an assistant summarizing books.
  Style: {{ style }}
  Language: {{ language }}

  Analyze the following content and produce a structured JSON summary:
  {{ file_content }}
  ```

#### 3.5.2 CLI interface

* `genie templates list`
* `genie templates show <name>`
* `genie templates run <name> --var style=detailed --file mybook.pdf`
* `genie templates new <name>` (optional helper to bootstrap a new file).

**Behavior:**

* Parse frontmatter with a YAML parser.
* Render body with Tera-like template engine:

  * Inject variables from `--var` flags.
  * For `file` variables, read file content and inject as `{{ file_content }}`.
* Run prompt through Gemini and print result (or save to file, depending on template config).

#### 3.5.3 GUI integration

* In Tauri UI:

  * Show Prompt Gallery list.
  * On selecting a prompt:

    * Auto-generate form from `input_variables`.
    * Provide file pickers for `type: file`.
  * On submit:

    * Call backend to execute prompt.
    * Show streaming output in UI.

---

### 3.6 HTTP API & LangChain compatibility

#### 3.6.1 OpenAI-style endpoint

* Base URL (configurable, default): `http://localhost:11435`
* Endpoint: `POST /v1/chat/completions`

**Expected request (subset of OpenAI spec):**

```json
{
  "model": "gemini-2.5-pro",
  "messages": [
    {"role": "system", "content": "You are..."},
    {"role": "user", "content": "Explain ..."}
  ],
  "max_tokens": 1024,
  "temperature": 0.7,
  "stream": false
}
```

**Behavior:**

* Map messages to a concatenated prompt string or a structured chat prompt depending on CLI capabilities.
* Invoke `gemini` CLI accordingly:

  * For non-streaming: `--output-format json`.
  * For streaming: `--output-format stream-json` (if supported) and forward tokens via chunked response.
* Track quota for each call.
* Return an OpenAI-like response JSON.

This allows LangChain to use `ChatOpenAI` pointing to Genie.

#### 3.6.2 Genie-internal endpoints

For internal UI and programmatic usage:

* `POST /v1/json` – same as `/chat/completions` but guarantees JSON output with optional schema.
* `POST /v1/docs/summarize` – PDF summarization.
* `POST /v1/repo/summary` – repo summary.
* `GET /v1/quota` – quota & usage stats.

---

### 3.7 RAG & Vector Store (later phase)

**Goal:** Let user build simple RAG-style apps using Genie as the LLM backend and an external or embedded vector store.

High-level requirements:

* `genie rag ingest <path> --collection <name>`

  * Chunk documents (using same text extraction pipeline).
  * Compute embeddings (embedding provider TBD, ideally no additional key unless user configures it).
  * Store in local vector store (e.g., Chroma or another embedded store).

* `genie rag query "question" --collection <name>`

  * Retrieve top-K relevant chunks.
  * Build Gemini prompt including those chunks.
  * Return answer with references.

RAG implementation is **Phase 3+**, not required for initial MVP.

---

### 3.8 macOS menu bar app & GUI behavior

Using Tauri:

* Provide a **tray / menu bar icon** with:

  * Current day’s usage (e.g., progress bar 0–1000).
  * Requests per minute status.
  * Button to open main Genie window.
  * Controls to:

    * Start/stop Genie daemon.
    * Change port and basic settings.

Main window:

* Tabs / “workspaces”:

  1. **Chat** – simple chat interface using Genie backend.
  2. **Docs** – drag-and-drop area for PDFs + job progress list.
  3. **Repo** – select repo path and run summary.
  4. **Prompts** – prompt gallery browser and editor.
  5. **Quota** – graphs & recent usage.

---

## 4. System Architecture & Implementation Notes

### 4.1 Process orchestration

* Use `tokio::process::Command` to spawn `gemini`:

  * Stdin: piped (to send prompts).
  * Stdout: piped (capture output).
  * Stderr: piped (capture errors and detect auth/quota issues).

* Wrap in `GeminiClient` struct:

  ```rust
  struct GeminiClient {
      binary_path: PathBuf,
      default_model: String,
      // …
  }

  impl GeminiClient {
      async fn call(&self, prompt: &PromptSpec) -> Result<GeminiResponse, GeminiError> { /* … */ }
  }
  ```

* Support both:

  * One-shot calls (spawn per request).
  * Possibly limited concurrent calls with semaphore.

### 4.2 State & conversation management

* For now, focus on **stateless** calls.
* If conversation mode is needed:

  * Genie maintains history and re-injects relevant messages into each call.
  * Structure prompts to maximize Gemini’s implicit context caching (static header + heavy context first, chat turns last).

### 4.3 Quota DB & data schema

Prefer SQLite via `sqlx` / `rusqlite`:

* `usage_events` table:

  ```sql
  CREATE TABLE usage_events (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      timestamp TEXT NOT NULL,
      model TEXT NOT NULL,
      kind TEXT NOT NULL, -- ask/json/pdf/repo/chat/etc
      prompt_chars INTEGER NOT NULL,
      response_chars INTEGER NOT NULL,
      approx_input_tokens INTEGER NOT NULL,
      approx_output_tokens INTEGER NOT NULL,
      success BOOLEAN NOT NULL,
      error_code TEXT
  );
  ```

* Helper functions:

  * `count_requests_since(ts)`
  * `count_requests_today()`
  * `sum_tokens_today()`

Approx tokens:

* `approx_tokens = (chars / 4)` or similar.

---

## 5. Configuration

Config file: `~/.genie/config.toml` (or JSON/YAML).

Example:

```toml
[gemini]
binary = "gemini"
default_model = "gemini-2.5-pro"

[server]
port = 11435
host = "127.0.0.1"

[quota]
per_minute = 60
per_day = 1000
reset_time = "00:00"

[logging]
level = "info"
```

CLI flags override config; config overrides defaults.

---

## 6. Phased Implementation Plan

### Phase 1 – Core Rust + CLI + quota (MVP)

* Implement `genie-core` and `genie-cli`.
* Features:

  * `genie ask`, `genie json`.
  * Quota DB & `genie quota status`.
  * Basic `genie up` daemon with minimal TUI.
  * HTTP API: `/v1/chat/completions` (non-streaming OK).
* Packaging:

  * `cargo install genie-cli`.
  * GitHub Releases with binaries for macOS & Linux.

### Phase 2 – PDF/Book & repo summary + prompt gallery

* Implement PDF extraction + chapter detection + chapter summaries.
* Commands:

  * `genie summarize-pdf`, `genie summarize-book`.
  * `genie repo-summary`.
* Implement prompt gallery (`*.prompt.md`) + `genie templates` commands.
* Expand HTTP API to support docs and repo endpoints.

### Phase 3 – Tauri UI & macOS menu bar

* Tauri app with:

  * Chat, Docs, Repo, Prompts, Quota views.
  * Tray icon with quotas & daemon control.
* Integrate with Genie backend (HTTP or Rust bindings).
* Add streaming to HTTP API if feasible from `gemini` CLI.

### Phase 4 – RAG / embeddings (optional)

* Design simple RAG interface and integrate a vector store.
* Provide `rag ingest` and `rag query` commands.
* Sample LangChain + Genie RAG demo project.

---