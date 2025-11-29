
```markdown
# phase_02_docs_repo_prompts.md

## Phase 2 – PDF/Book Summarization, Repo Summary & Prompt Gallery

### Goal

Extend Genie with:

- PDF / book summarization (single-doc and chapter-based).
- Repo summarization (`genie repo-summary`).
- Prompt gallery using `.prompt.md` files.
- HTTP endpoints for docs & repo operations.

Phase 2 builds on Phase 1 (core CLI, quota, HTTP).

---

## Milestones

- **M1:** PDF extraction + text model.
- **M2:** Chapter detection algorithm (heuristic + Gemini).
- **M3:** `summarize-pdf` & `summarize-book` CLI commands.
- **M4:** Repo summarization (`repo-summary`).
- **M5:** Prompt gallery engine (`.prompt.md` + templating).
- **M6:** `genie templates` CLI.
- **M7:** HTTP endpoints for docs & repo.

---

## Task Breakdown

### 1. PDF Extraction & Text Model

**Goal:** Read PDFs into structured Rust types with page and formatting info.

- [x] **1.1** Add PDF dependency (e.g., `lopdf` or equivalent).
- [x] **1.2** Define core PDF types in `genie-core::pdf`:

  ```rust
  struct PdfDocument {
      path: PathBuf,
      pages: Vec<PdfPage>,
  }

  struct PdfPage {
      index: u32,
      text_blocks: Vec<TextBlock>,
  }

  struct TextBlock {
      text: String,
      font_size: f32,
      bold: bool,
      // Optional: x/y position, font name, etc.
  }
````

* [x] **1.3** Implement `fn load_pdf(path: &Path) -> Result<PdfDocument, PdfError>`.
* [x] **1.4** Implement basic heuristics to merge adjacent text segments into logical `TextBlock`s.
* [x] **1.5** Provide helper methods:

  * [x] `fn full_text(&self) -> String`
  * [x] `fn pages_text_range(&self, start: u32, end: u32) -> String`

#### Acceptance Criteria

* [x] Given a sample PDF, `load_pdf` returns a non-empty `PdfDocument` with at least one page and text blocks.
* [x] `full_text()` returns text that, when manually inspected for a test file, roughly matches the visible text.
* [x] No panics on corrupted or odd PDFs; errors are gracefully reported.

#### Auto-Testing Prompt

```text
You are an automated tester for Genie's PDF extraction.

1. Use a small sample PDF (2-3 pages of simple text).
2. Call `load_pdf(path)` and inspect:
   - `pages.len()` > 0
   - `pages[0].text_blocks` is non-empty.
3. Compare `full_text()` to the known contents of the sample PDF (approximate match is fine).
4. Attempt to load a non-PDF file or corrupted PDF and verify `PdfError` is returned.

Respond with JSON:
- `sample_pdf_pages`: number
- `first_page_blocks`: number
- `full_text_excerpt`: string
- `error_handling_verified`: bool
```

---

### 2. Chapter Detection (Heuristic + Gemini)

**Goal:** Detect chapter boundaries using font heuristics, then refine via Gemini.

* [x] **2.1** Define `ChapterCandidate` and `Chapter` structs:

  ```rust
  struct ChapterCandidate {
      page_index: u32,
      block_index: usize,
      title: String,
      font_size: f32,
      bold: bool,
  }

  struct Chapter {
      id: u32,
      title: String,
      start_page: u32,
      end_page: u32,
  }
  ```

* [x] **2.2** Implement heuristic detection:

  * [x] Compute font size histogram for entire document.
  * [x] Infer body font size as the most frequent size.
  * [x] Collect `ChapterCandidate`s where:

    * `font_size` is significantly greater than body size (e.g., +2pt or configurable factor).
    * Or `bold == true`.
  * [x] Optionally apply regex filtering:

    * Titles matching `^Chapter \d+`, `^Section \d+`, roman numerals, etc.

* [x] **2.3** Implement function:

  ```rust
  fn detect_chapter_candidates(doc: &PdfDocument) -> Vec<ChapterCandidate>
  ```

* [x] **2.4** Implement Gemini refinement:

  ```rust
  async fn refine_chapters_with_gemini(
      client: &GeminiClient,
      candidates: &[ChapterCandidate],
      maybe_toc_text: Option<String>,
  ) -> Result<Vec<Chapter>, GeminiError>
  ```

  * [x] Build a compact prompt to Gemini:

    * Include ToC (if detectable) + candidate list.
    * Ask for JSON with chapters `[ { "title", "start_page", "end_page" } ]`.
  * [x] Parse and validate JSON into `Chapter`.

* [x] **2.5** Implement a unified function:

  ```rust
  async fn detect_chapters(doc: &PdfDocument, client: &GeminiClient) -> Result<Vec<Chapter>, Error>
  ```

#### Acceptance Criteria

* [x] On a test PDF with clear “Chapter 1, 2, 3” headings, `detect_chapter_candidates` produces candidates with correct page indices.
* [x] `refine_chapters_with_gemini` returns a list of `Chapter` with non-overlapping sequential ranges that cover the book.
* [x] On a PDF without obvious chapters, function might return 0 or 1 chapter gracefully (fallback behavior documented).

#### Auto-Testing Prompt

```text
You are an automated tester verifying Genie's chapter detection.

Assume a test PDF where:
- Pages 1–3: "Chapter 1"
- Pages 4–6: "Chapter 2"

1. Run `detect_chapter_candidates` and list all candidates.
2. Pass candidates to `refine_chapters_with_gemini` (mock Gemini if needed to return correct ranges).
3. Verify the resulting chapters:
   - Two entries: Chapter 1 (1-3), Chapter 2 (4-6).
   - Titles are non-empty.

Return JSON:
- `candidate_count`: number
- `chapters_detected`: array of `{title, start_page, end_page}`
- `structure_valid`: bool
```

---

### 3. Summarization Pipeline: `summarize-pdf` & `summarize-book`

**Goal:** Provide CLI commands to summarize PDFs.

#### 3.1 Shared data models

* [x] Define summary structs in `genie-core::docs`:

  ```rust
  struct ChapterSummary {
      chapter_id: u32,
      title: String,
      summary: String,
      key_points: Vec<String>,
      important_terms: Vec<String>,
      questions_for_reflection: Vec<String>,
  }

  struct BookSummary {
      title: Option<String>,
      author: Option<String>,
      chapters: Vec<ChapterSummary>,
      global_summary: String,
      reading_roadmap: Vec<String>,
  }

  struct DocumentSummary {
      title: Option<String>,
      summary: String,
      key_points: Vec<String>,
  }
  ```

#### 3.2 `genie summarize-pdf <file>`

* [x] Add subcommand to CLI:

  * [x] Options:

    * `--style`
    * `--language`
    * `--format json|markdown|both`
    * `--out <path>`
    * `--prompt-id <prompt>` or `--prompt-file`
* [x] Implementation:

  * [x] Load `PdfDocument`.
  * [x] Extract full text.
  * [x] Build summarization prompt, optionally using templates (if already wired).
  * [x] Call Gemini to get `DocumentSummary` JSON.
  * [x] Serialize to JSON file if requested.
  * [x] Generate Markdown if requested:

    * Title, summary, bullet points, etc.

#### 3.3 `genie summarize-book <file>`

* [x] Add subcommand with options:

  * [x] `--style`, `--language`, `--format`, `--out`, `--prompt-id`, etc.
* [x] Implementation pipeline:

  * [x] Load `PdfDocument`.
  * [x] Detect chapters (`detect_chapters`).
  * [x] For each `Chapter`:

    * Extract text range via `pages_text_range`.
    * If text exceeds `max_chunk_tokens`, split into smaller chunks.
    * For each chunk, call Gemini with a summarization prompt.
    * Aggregate chunk-level results into one `ChapterSummary` via another Gemini call.
  * [x] Once all chapters summarized:

    * Call Gemini with all chapter summaries to produce `global_summary` + `reading_roadmap`.
  * [x] Combine into `BookSummary`.
  * [x] Save JSON and/or Markdown.

#### Acceptance Criteria

* [x] `genie summarize-pdf sample.pdf` runs to completion and creates an output file.
* [x] `genie summarize-book sample_book.pdf` runs to completion and:

  * Produces a JSON file matching `BookSummary` structure.
  * Produces readable Markdown with chapter sections.
* [x] For a small test book where content is known, summaries are coherent and references to chapters match roughly the original.

#### Auto-Testing Prompt

```text
You are an automated end-to-end tester for Genie's PDF summarization.

1. Run: `genie summarize-pdf tests/data/sample.pdf --format json --out sample_summary.json`
   - Parse the output JSON and verify it includes keys: `title`, `summary`, `key_points`.

2. Run: `genie summarize-book tests/data/sample_book.pdf --format json --out book_summary.json`
   - Parse output JSON and verify:
     - It has `chapters` array with length > 0.
     - Each chapter contains `title`, `summary`, and `key_points`.
     - `global_summary` is a non-empty string.

3. Check that both commands created files in the expected locations.

Return a JSON report with:
- `summarize_pdf_passed`: bool
- `summarize_book_passed`: bool
- Structural validation errors if any.
```

---

### 4. Repo Summary (`genie repo-summary`)

**Goal:** Summarize a code repository’s structure and responsibilities.

* [x] **4.1** Implement `genie-core::repo` module:

  * [x] Use `walkdir` to scan directory.
  * [x] Respect `.gitignore` (using `ignore` crate).
  * [x] Identify text files by extension (`.rs`, `.ts`, `.js`, `.py`, etc.).
  * [x] Group files by directory & language.

* [x] **4.2** Define data types:

  ```rust
  struct FileSnippet {
      path: PathBuf,
      language: String,
      content: String, // truncated if large
  }

  struct RepoChunk {
      id: u32,
      description: String,
      files: Vec<FileSnippet>,
  }

  struct RepoSummary {
      overview: String,
      modules: Vec<ModuleSummary>,
  }

  struct ModuleSummary {
      path: String,
      description: String,
      key_files: Vec<String>,
  }
  ```

* [x] **4.3** Implement chunking:

  * [x] Concatenate files into `RepoChunk`s that roughly fit within context limits.
  * [x] For each chunk, call Gemini with a prompt:

    * “Summarize the purpose of these files and their relationships.”
  * [x] Aggregate chunk-level summaries into `RepoSummary` via another Gemini call.

* [x] **4.4** Add CLI command `genie repo-summary [path]`:

  * [x] Options:

    * `--out <path>`
    * `--format json|markdown`
    * `--max-files` (optional limit for tests)
  * [x] Output JSON and/or Markdown.

#### Acceptance Criteria

* [x] Running `genie repo-summary .` on a small test repo:

  * Succeeds without panic.
  * Outputs a JSON or Markdown summary.
* [x] For a known simple repo, summary mentions major directories and modules correctly (manually verifiable).
* [x] `.gitignore` is respected (ignored files are not read).

#### Auto-Testing Prompt

```text
You are an automated tester for Genie's repo summary feature.

1. Use a sample repo with:
   - `src/lib.rs`
   - `src/main.rs`
   - `tests/` directory.
2. Run: `genie repo-summary path/to/sample_repo --format json --out repo_summary.json`
3. Parse `repo_summary.json` and verify:
   - The `overview` string is non-empty.
   - `modules` is a non-empty array.
   - At least one module mentions `src` or main entry file by path.

Return a JSON object:
- `summary_file_exists`: bool
- `overview_non_empty`: bool
- `module_count`: number
- Example module entry
- `tests_passed`: bool
```

---

### 5. Prompt Gallery: `.prompt.md` + Templating

**Goal:** Allow reusable prompts with parameters and optional file content injection.

#### 5.1 Storage Convention

* [x] Store templates in `~/.genie/prompts/*.prompt.md`.
* [x] Use YAML frontmatter + Markdown body e.g.:

  ```yaml
  name: "book-summary"
  description: "Summarize a book with chapters"
  model: "gemini-2.5-pro"
  input_variables:
    - name: "style"
      description: "Summary style"
      default: "concise"
    - name: "language"
      description: "Language"
      default: "en"
    - name: "file"
      type: "file"
      description: "Book file path"
  ---
  You are a summarizer.
  Style: {{ style }}
  Language: {{ language }}

  Content:
  {{ file_content }}
  ```

#### 5.2 Parser & Template Engine

* [x] **5.2.1** Add YAML parser (`serde_yaml`) and frontmatter splitting logic.

* [x] **5.2.2** Define internal structs:

  ```rust
  struct PromptTemplate {
      name: String,
      description: String,
      model: String,
      input_variables: Vec<InputVar>,
      body: String,
  }

  struct InputVar {
      name: String,
      description: String,
      var_type: InputVarType, // String | File | Enum etc.
      default: Option<String>,
  }
  ```

* [x] **5.2.3** Use `tera` (or similar) for interpolation:

  * [x] Render context from supplied variables and file contents.
  * [x] Special handling:

    * For `type: file`, read file and expose as `file_content` or `<name>_content`.

* [x] **5.2.4** Implement loader:

  ```rust
  fn load_prompt_templates() -> Result<Vec<PromptTemplate>, Error>
  fn find_template_by_name(name: &str) -> Option<PromptTemplate>
  ```

#### 5.3 CLI for templates

* [x] `genie templates list`

  * [x] Prints template names and descriptions.
* [x] `genie templates show <name>`

  * [x] Prints frontmatter and body.
* [x] `genie templates run <name> --var key=value --file key=path`

  * [x] Parse template.
  * [x] Build variable map.
  * [x] Render body.
  * [x] Call Gemini with rendered prompt and template’s `model`.
  * [x] Output response (text or json depending on template).

#### Acceptance Criteria

* [x] At least one example `.prompt.md` lives in repo’s test fixtures.
* [x] `genie templates list` shows installed templates.
* [x] `genie templates run example-template --var style=detailed --file file=tests/data/sample.txt` produces a response.
* [x] If required input variables are missing, `run` fails with a clear error.

#### Auto-Testing Prompt

```text
You are an automated tester for Genie's prompt gallery.

1. Place a test prompt file at `~/.genie/prompts/test.prompt.md` with:
   - name: "test"
   - one string variable `who` with default "world".
   - body: "Hello, {{ who }}!"
2. Run: `genie templates list` and verify "test" appears.
3. Run: `genie templates run test --var who=GenieAgent`.
   - Capture the rendered prompt sent to Gemini (mock Gemini if needed).
   - Verify the rendered string contains "Hello, GenieAgent!".

Return JSON:
- `template_list_contains_test`: bool
- `rendered_prompt_excerpt`: string
- `tests_passed`: bool
```

---

### 6. HTTP Endpoints for Docs & Repo

**Goal:** Make PDF and repo functionality usable over HTTP for other tools.

#### 6.1 Docs endpoint

* [x] Add `POST /v1/docs/summarize`:

  * Body (multipart or JSON depending on design):

    * If multipart: file upload + JSON options.
    * If JSON + file path: require path on local filesystem (trusted environment).
  * Options:

    * `mode: "pdf" | "book"`
    * `style`, `language`, `format`.
* [x] Handler behavior:

  * [x] Call respective core functions (`summarize_pdf` or `summarize_book`).
  * [x] Return JSON summary directly in response (do not only write file).

#### 6.2 Repo endpoint

* [x] Add `POST /v1/repo/summary`:

  * JSON body:

    * `path: String`
    * optional options for format etc.
  * Response:

    * JSON `RepoSummary`.

#### Acceptance Criteria

* [x] `POST /v1/docs/summarize` with valid payload returns JSON with summary key fields.
* [x] `POST /v1/repo/summary` returns `RepoSummary` structure.
* [x] Errors (invalid file/path) are returned as helpful JSON errors with appropriate HTTP status.

#### Auto-Testing Prompt

```text
You are an automated HTTP tester for Genie's docs and repo endpoints.

1. Ensure Genie server is running (`genie up`).
2. For docs:
   - Send `POST /v1/docs/summarize` with mode "pdf" and path to a sample PDF on disk.
   - Verify response:
     - HTTP 200.
     - Contains `summary` and `key_points`.

3. For repo:
   - Send `POST /v1/repo/summary` with `{ "path": "path/to/sample_repo" }`.
   - Verify:
     - HTTP 200.
     - Response has `overview` and `modules` array.

Return JSON:
- `docs_endpoint_passed`: bool
- `repo_endpoint_passed`: bool
- Example response snippets.
```

---

### 7. Documentation & Examples for Phase 2

**Goal:** Make new features discoverable and testable.

* [x] **7.1** Update README:

  * [x] Add usage examples for:

    * `summarize-pdf`, `summarize-book`.
    * `repo-summary`.
    * `templates list/run`.
  * [x] Document the `.prompt.md` spec with frontmatter example.
* [x] **7.2** Add sample prompt templates in a `examples/prompts` folder.
* [x] **7.3** Add example commands in a `docs/USAGE.md` for Phase 2 features.
* [x] **7.4** Add integration tests or scripted examples (e.g., shell scripts) that run end-to-end flows with test PDFs/repos.

#### Acceptance Criteria

* [x] README clearly explains how to run new commands.
* [x] At least one working example prompt file is referenced in the docs.
* [x] A new contributor can reproduce PDF and repo summaries by following docs only.

#### Auto-Testing Prompt

```text
You are an automated documentation tester.

1. Open the README and docs/USAGE.md and extract:
   - The sections describing `summarize-pdf`, `summarize-book`, `repo-summary`, and `templates run`.
2. Verify:
   - Each documented command includes a full example that can be copy-pasted.
   - All command names and flags match the implemented CLI (no mismatches).

Return JSON:
- `commands_documented`: list of command names found.
- `missing_documentation`: list of commands implemented but not documented (if any).
- `docs_consistent`: bool
```

```
::contentReference[oaicite:0]{index=0}
```
