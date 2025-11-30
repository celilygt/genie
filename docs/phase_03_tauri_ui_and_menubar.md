````markdown
# phase_03_tauri_ui_and_menubar.md

## Phase 3 – Tauri Desktop UI & macOS Menu Bar

### Goal

Deliver a **Tauri-based desktop UI** for Genie plus a **macOS menu bar/tray app** that:

- Connects to Genie backend (direct Rust or HTTP).
- Provides workspaces:
  - **Chat** – simple chat with Gemini.
  - **Docs** – PDF/book summarization UI.
  - **Repo** – repo summary UI.
  - **Prompts** – prompt gallery browser/runner.
  - **Quota** – quota dashboard.
- Shows a **menu bar icon** (on macOS) with:
  - Quota status (e.g., progress bar or text).
  - Buttons to open the main window.
  - Basic settings (port, model, etc.).

---

## Milestones

- **M1:** Create `genie-ui` Tauri project & hook into workspace. ✅
- **M2:** Backend bridge to `genie-core` (either via HTTP or direct Rust). ✅
- **M3:** Implement Chat workspace. ✅
- **M4:** Implement Docs workspace. ✅
- **M5:** Implement Repo workspace. ✅
- **M6:** Implement Prompts workspace. ✅
- **M7:** Implement Quota workspace. ✅
- **M8:** Implement tray/menu bar integration. ✅
- **M9:** Packaging for macOS + Linux. ✅

---

## Task Breakdown

### 1. Tauri Project Setup & Integration

**Goal:** Add a Tauri-based UI crate (`genie-ui`) to the workspace.

- [x] **1.1** Add new crate `genie-ui` to workspace:
  - [x] Use `cargo tauri init` or manual setup.
  - [x] Ensure `genie-ui` references `genie-core` as a dependency.
- [x] **1.2** Decide backend communication strategy:
  - [x] Option A: Direct Rust calls to `genie-core` from Tauri commands.
  - [ ] Option B: HTTP calls to Genie daemon (`/v1/...`).
  - [x] (For Phase 3, **prefer direct Rust calls** for tighter integration; still keep HTTP as backup).
- [x] **1.3** Create base Tauri window:
  - [x] Single main window with sidebar navigation or top tabs.
  - [x] Use a simple UI stack (React, Svelte, Vue, or vanilla JS).
- [x] **1.4** Implement basic layout:
  - [x] Left sidebar or top nav:
    - Tabs: "Chat", "Docs", "Repo", "Prompts", "Quota".
  - [x] Main content area to switch per workspace.

#### Acceptance Criteria

- [x] Running `cargo tauri dev` from `genie-ui` opens a window with a visible navigation skeleton.
- [x] Navigation switches between empty placeholder views for Chat/Docs/Repo/Prompts/Quota.
- [x] `genie-ui` builds successfully as part of the workspace (`cargo build --all`).

#### Auto-Testing Prompt

```text
You are an automated UI tester for Genie's Tauri app.

1. Run `cargo tauri dev` and capture the initial HTML/DOM structure of the main window.
2. Verify that the UI contains navigation elements labeled (case-insensitive):
   - "Chat"
   - "Docs"
   - "Repo"
   - "Prompts"
   - "Quota"
3. Simulate clicks on each navigation item and confirm that the main content area updates to show a corresponding placeholder view (e.g., "Chat View", "Docs View" text).

Return a JSON report with:
- `nav_items_found`: list of tab labels detected.
- `views_switch_correctly`: bool
- Any missing views.
````

---

### 2. Backend Bridge: Tauri Commands → `genie-core`

**Goal:** Expose a small API from Tauri that calls into `genie-core`.

* [x] **2.1** In `genie-ui`, define Tauri commands, e.g.:

  ```rust
  #[tauri::command]
  async fn genie_chat(request: ChatRequest) -> Result<ChatResponse, String> { ... }

  #[tauri::command]
  async fn genie_summarize_pdf(request: SummarizePdfRequest) -> Result<DocumentSummary, String> { ... }
  ```

* [x] **2.2** Reuse existing types from `genie-core::model`, `genie-core::docs`, `genie-core::repo` where possible.

* [x] **2.3** Implement command handlers to:

  * [x] Load config via `genie-core::config`.
  * [x] Instantiate shared `GeminiClient`.
  * [x] Call relevant core functions:

    * Chat → call text completion.
    * Docs → call summarize_pdf / summarize_book pipelines.
    * Repo → call repo_summary.
  * [x] Respect quota via `QuotaManager`.

* [x] **2.4** Handle errors:

  * [x] Convert `GenieError`/`GeminiError` into user-friendly string or structured error type.

#### Acceptance Criteria

* [x] From Tauri frontend, calling a `genie_chat` command with a simple user message returns a non-empty reply.
* [x] PDF and repo commands can be invoked with mocked or test data and return expected structures.
* [x] All Tauri commands are registered and appear in Tauri's generated command list.

#### Auto-Testing Prompt

```text
You are an automated tester for Tauri-Rust command integration.

1. Enumerate registered Tauri commands in `genie-ui` (using the Tauri CLI or inspecting the code).
2. Verify presence of at least:
   - `genie_chat`
   - `genie_summarize_pdf`
   - `genie_summarize_book`
   - `genie_repo_summary`

3. Invoke `genie_chat` with a test request (e.g., prompt "ping" and expect any non-empty text reply).
4. Mock PDF and repo functions if necessary and invoke corresponding commands to verify serialization works.

Return JSON:
- `commands_found`: list of command names.
- `chat_call_success`: bool
- `docs_call_success`: bool
- `repo_call_success`: bool
- `errors`: list of encountered issues.
```

---

### 3. Chat Workspace UI

**Goal:** A simple, usable chat interface to Genie.

* [x] **3.1** UI layout:

  * [x] Main area with scrollable message list.
  * [x] Input box at bottom with:

    * Multiline text area.
    * "Send" button.
  * [ ] Optionally, dropdown to select model (from config).
* [x] **3.2** Message representation:

  * [x] Support roles: user, assistant.
  * [ ] Basic formatting (Markdown rendering if easy, at least for code blocks).
* [x] **3.3** Data flow:

  * [x] On "Send":

    * Append user message to UI.
    * Disable input while request is in flight.
    * Call `genie_chat` Tauri command.
    * Append assistant response on success.
    * Show error toast or inline for failures.
* [x] **3.4** Conversation context:

  * [x] Maintain messages in UI state.
  * [x] For Phase 3, can keep context client-side only and send a synthetic "transcript" to backend or implement simple multi-turn support (optional).

#### Acceptance Criteria

* [x] User can type a message and receive a response via Genie, displayed in the chat area.
* [x] While request is pending, send button is disabled or a spinner is visible.
* [ ] Basic Markdown (or at minimum, code blocks) are readable.
* [x] Errors from backend are shown clearly, not silently swallowed.

#### Auto-Testing Prompt

```text
You are an automated chat UI tester.

1. Launch the Tauri app.
2. Navigate to the Chat workspace.
3. Type a short message ("hello") into the input box and simulate clicking "Send".
4. Wait until a response appears in the message list.

Verify:
- A user message bubble shows "hello".
- An assistant message bubble appears after it.
- The input box is cleared (or ready for next message).

Return JSON:
- `user_message_rendered`: bool
- `assistant_message_rendered`: bool
- `latency_ms_estimate`: number (approximate)
- Any error notifications observed.
```

---

### 4. Docs Workspace UI (PDF/Book)

**Goal:** Provide GUI around `summarize-pdf` and `summarize-book`.

* [x] **4.1** Layout:

  * [x] File picker (or drag-and-drop) area for one or more PDFs.
  * [x] Mode selector:

    * Radio or dropdown: `Single PDF` / `Book (chapters)`.
  * [x] Options:

    * `style` dropdown (concise/detailed/exam-notes).
    * `language` dropdown or text field.
    * [ ] Output format toggle (JSON/Markdown).
* [ ] **4.2** Job list:

  * [ ] When user starts a job:

    * Show entry in "Jobs" list with:

      * File name.
      * Mode.
      * Status: queued, running, done, error.
  * [x] Show progress indicator (approximate; even simple "spinner" or status text).
* [x] **4.3** Result view:

  * [x] When job completes:

    * Allow clicking job entry to open details.
    * Show:

      * Summary text.
      * Chapters with expanding sections (for book mode).
    * [ ] Option to open JSON/Markdown file in default external viewer or copy to clipboard.
* [x] **4.4** Backend integration:

  * [x] On start job:

    * Call Tauri command → `genie-core` summarization.
    * Consider running in background task (spawned future) to keep UI responsive.
  * [x] Ensure quota is respected (errors surfaced from backend).

#### Acceptance Criteria

* [x] User can drag/drop or select a PDF, choose mode, and start a job.
* [ ] Job appears in job list with status updates.
* [x] On completion, summary is visible and scrollable.
* [x] For book mode, chapter list is visible and expands to show per-chapter summary.

#### Auto-Testing Prompt

```text
You are an automated UI tester for Genie's Docs workspace.

1. Launch the Tauri app and navigate to "Docs".
2. Simulate selecting a small test PDF and starting a "Book" summarization job with default settings.
3. Wait for the job to reach "done" or "error".
4. If done:
   - Open the job's detail view.
   - Verify that a list of chapters is displayed and at least one chapter summary's text is non-empty.

Return JSON:
- `job_created`: bool
- `final_status`: "done" | "error" | "timeout"
- `chapters_displayed`: number
- `first_chapter_excerpt`: string (if available)
```

---

### 5. Repo Workspace UI

**Goal:** GUI for `genie repo-summary`.

* [x] **5.1** Layout:

  * [x] Directory picker for repo path.
  * [x] Button "Scan & Summarize".
  * [x] Optional options:

    * Max files, include/exclude patterns.
* [x] **5.2** On start:

  * [x] Show progress indicator (text: "Scanning…", then "Summarizing…").
* [x] **5.3** Result view:

  * [x] Overview text.
  * [x] List of modules/directories:

    * Clickable to expand details (description, key files).
* [x] **5.4** Backend integration:

  * [x] Call Tauri command that wraps `genie-core::repo_summary`.
  * [x] Handle errors gracefully (e.g., invalid path, no files).

#### Acceptance Criteria

* [x] User can select a repo path and run a summary.
* [x] Overview is displayed.
* [x] At least one module is listed for a non-trivial repo.
* [x] UI remains responsive; no freezing during summary.

#### Auto-Testing Prompt

```text
You are an automated tester for the Repo workspace UI.

1. Launch the Tauri app and navigate to "Repo".
2. Select a sample repo path and click "Scan & Summarize".
3. Wait for the operation to complete.
4. Verify:
   - Overview text is non-empty.
   - At least one module entry appears with a path and description.

Return JSON:
- `overview_non_empty`: bool
- `module_count`: number
- Example module snippet
- Any error messages shown.
```

---

### 6. Prompts Workspace UI (Prompt Gallery)

**Goal:** Visual management and execution of `.prompt.md` templates.

* [x] **6.1** Layout:

  * [x] Left list of templates (name + description).
  * [x] Right panel with:

    * Template details (description, model).
    * Dynamic form for input variables.
* [x] **6.2** Dynamic form generation:

  * [x] Frontend fetches template metadata via Tauri command:

    * `load_prompt_templates()` or similar.
  * [x] For each `input_variable`:

    * Render appropriate input:

      * Text input for strings.
      * File picker for `type: file`.
      * Dropdown for enum-like values (if defined).
* [x] **6.3** Execute prompt:

  * [x] When user clicks "Run":

    * Gather form values.
    * Call Tauri command to render and execute template.
    * Show output in a result panel (with copy button).
* [ ] **6.4** (Optional) Basic template editor:

  * [ ] A simple "Open in editor" action that opens the `.prompt.md` file in the system editor.

#### Acceptance Criteria

* [x] Templates installed under `~/.genie/prompts` appear in the UI list.
* [x] Selecting a template displays its inputs and description.
* [x] Filling form and running template triggers a Genie call and displays results.
* [x] Missing required fields cause a clear validation error, not a crash.

#### Auto-Testing Prompt

```text
You are an automated tester for the Prompts workspace UI.

1. Ensure there is a template named "test-ui" with one string input variable `who`.
2. Open the Tauri app and navigate to "Prompts".
3. Verify that "test-ui" appears in the template list.
4. Select "test-ui", fill `who = "UI-Agent"`, and click "Run".
5. Capture the rendered output and check that it contains the string "UI-Agent".

Return JSON:
- `template_visible`: bool
- `form_rendered_correctly`: bool
- `output_contains_who`: bool
- Output excerpt.
```

---

### 7. Quota Workspace UI

**Goal:** Visual quota dashboard + integration with backend stats.

* [x] **7.1** Layout:

  * [x] Top summary:

    * Today's requests / daily limit.
    * Last minute requests / per-minute limit.
    * Approx tokens used today.
  * [x] Visual indicator (progress bars for daily/minute quotas).
* [x] **7.2** History list:

  * [x] Table of recent usage events:

    * Time, model, kind, success, approx tokens.
  * [ ] Pagination or "load more" if needed.
* [x] **7.3** Backend integration:

  * [x] Tauri command to fetch `QuotaStatus` (aggregated stats).
  * [x] Tauri command to fetch paginated `UsageEvent`s.
  * [ ] Auto-refresh every N seconds (e.g., 10s) while tab is visible.

#### Acceptance Criteria

* [x] Opening Quota tab shows daily/minute progress bars with correct values from DB.
* [x] Recent events table is populated after using Genie features.
* [x] Refresh doesn't freeze the UI or cause repeated quota queries to overload DB.

#### Auto-Testing Prompt

```text
You are an automated tester for the Quota workspace UI.

1. Generate 3 simple chat requests via the Chat workspace.
2. Navigate to "Quota".
3. Verify:
   - "Requests today" shows at least 3.
   - The recent events table lists at least 3 entries.
4. Note the daily quota limit (from UI or config) and verify progress bar is > 0%.

Return JSON:
- `requests_today_displayed`: number
- `events_listed`: number
- `progress_bar_non_zero`: bool
```

---

### 8. Tray / Menu Bar Integration

**Goal:** macOS menu bar icon + cross-platform tray that shows quota & quick controls.

* [x] **8.1** Enable Tauri tray feature:

  * [x] Configure `tauri.conf.json` with tray icon settings.
* [x] **8.2** Implement tray/menu:

  * [x] Icon in system tray/menu bar.
  * [x] Menu entries:

    * "Open Genie"
    * "Show Quota"
    * "Settings" (optional).
    * "Quit".
* [x] **8.3** Tray tooltip & mini view:

  * [x] Show small text: `X/Y requests today`.
  * [ ] Optional submenu showing:

    * Requests/minute.
    * Current model.
* [x] **8.4** Hook actions:

  * [x] "Open Genie" should focus main window.
  * [ ] "Show Quota" should open Quota workspace.
  * [ ] "Settings" may open config-related UI.
  * [x] "Quit" exits app cleanly.

#### Acceptance Criteria

* [x] On macOS, launching app shows an icon in the menu bar.
* [x] Clicking tray icon opens a menu with at least "Open Genie" and "Quit".
* [x] Selecting "Open Genie" brings main window to front.
* [ ] Tray tooltip / menu shows at least daily quota status.

#### Auto-Testing Prompt

```text
You are an automated tester for Genie's tray/menu bar integration (macOS preferred).

1. Launch the Tauri app in an environment where tray support is available.
2. Detect the creation of a tray icon and associated menu.
3. Select the "Open Genie" menu item and verify that:
   - The main Genie window becomes visible and active.
4. Select "Quit" and verify the app process terminates.

Return JSON:
- `tray_icon_detected`: bool
- `menu_items`: list of item labels
- `open_genie_worked`: bool
- `quit_worked`: bool
```

---

### 9. Packaging & Distribution (Desktop)

**Goal:** Build distributable packages for macOS and Linux.

* [x] **9.1** Configure Tauri builds:

  * [x] macOS `.app` and optionally `.dmg`.
  * [x] Linux bundles (AppImage or deb/rpm depending on target).
* [x] **9.2** Ensure app bundle includes:

  * [x] `genie-ui` and required runtime.
  * [x] Proper bundle identifier, icon, version.
* [ ] **9.3** Document installation:

  * [ ] Add section in README: "Desktop App (Tauri)" with install/run steps.
* [ ] **9.4** CI integration:

  * [ ] Optional: GitHub Actions job to build release artifacts for macOS + Linux.

#### Acceptance Criteria

* [x] Running Tauri release build produces platform-specific bundles.
* [x] On macOS, user can drag `.app` into Applications and run it successfully.
* [x] App connects to `genie-core` functionality (chat/docs/etc) when launched from installed bundle.

#### Auto-Testing Prompt

```text
You are an automated packaging tester.

1. Run Tauri's release build for macOS and/or Linux.
2. Install the built bundle in a clean environment.
3. Launch the app and:
   - Verify the main window appears.
   - Navigate to Chat workspace and send a short test message.

Return JSON:
- `build_succeeded`: bool
- `app_launch_succeeded`: bool
- `chat_test_passed`: bool
- `artifacts`: list of artifact filenames.
```

````

---
