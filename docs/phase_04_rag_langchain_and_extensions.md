
```markdown
# phase_04_rag_langchain_and_extensions.md

## Phase 4 – RAG, LangChain Integration & Advanced Extensions

### Goal

Add **retrieval-augmented generation (RAG)** capabilities and deepen **LangChain / OpenAI API compatibility**, turning Genie into a more complete backend for building AI applications.

Key outcomes:

- Simple **vector store** & collection management for local documents.
- `genie rag` CLI commands (ingest, list, query).
- HTTP endpoints for RAG queries.
- Improved OpenAI compatibility (streaming, additional fields).
- Clear examples showing **LangChain** using Genie as a backend.

> Note: Phase 4 is more advanced/optional; it assumes Phase 1–3 are stable.

---

## Milestones

- **M1:** OpenAI API compatibility enhancements (streaming, extra params).
- **M2:** RAG data model & vector store integration.
- **M3:** CLI commands: `rag init`, `rag ingest`, `rag query`, `rag ls`.
- **M4:** HTTP endpoints for RAG.
- **M5:** LangChain integration examples (Python).
- **M6:** Documentation & sample apps.

---

## Task Breakdown

### 1. OpenAI API Compatibility Enhancements

**Goal:** Make `/v1/chat/completions` closer to the OpenAI spec + support streaming.

- [ ] **1.1** Extend `ChatCompletionRequest` model:
  - [ ] Support optional fields:
    - `top_p`, `presence_penalty`, `frequency_penalty` (even if just stored for future).
    - `stop`, `n`, `user` (may be ignored or lightly used).
- [ ] **1.2** Implement `stream: true` support:
  - [ ] Use `gemini` CLI’s streaming mode (`--output-format stream-json` if available).
  - [ ] Translate Gemini’s streaming events into OpenAI-style chunks:
    - `data: { "choices": [ { "delta": { "content": "..." } } ] }`
  - [ ] Use HTTP chunked Transfer-Encoding.
- [ ] **1.3** Error shapes:
  - [ ] Wrap errors into JSON matching `{"error": {"message": "...", "type": "...", ...}}`.

- [ ] **1.4** Compatible headers & behavior:
  - [ ] Accept `Authorization: Bearer <fake-key>` for LangChain (not enforced).
  - [ ] Ensure CORS if needed (for browser-based clients).

#### Acceptance Criteria

- [ ] LangChain’s `ChatOpenAI` can talk to Genie using `stream=True` and receive token streams.
- [ ] Unsupported fields are safely ignored with no crashes.
- [ ] Errors from Genie are surfaced as proper OpenAI-like error objects.

#### Auto-Testing Prompt

```text
You are an automated API compatibility tester.

1. Start Genie's HTTP server.
2. Send a non-streaming POST to `/v1/chat/completions` with some optional fields (`top_p`, `stop`) and verify:
   - Response status 200.
   - Response adheres to OpenAI structure: `id`, `object`, `choices`, etc.

3. Send a streaming POST with `"stream": true` and read multiple chunks:
   - Verify each chunk begins with "data:" and contains a JSON with `choices[0].delta.content`.

4. Send a request that triggers an error (e.g., invalid payload) and verify:
   - Response has an `error` object with `message` and `type`.

Return JSON:
- `non_streaming_ok`: bool
- `streaming_ok`: bool
- `error_shape_ok`: bool
- Example chunk and error payload.
````

---

### 2. RAG Data Model & Vector Store

**Goal:** Design and implement a simple, local RAG subsystem.

**Implementation note:**
We’ll define generic traits for embedding providers and vector stores, so you can plug in different backends later (e.g., local ONNX model, external embedding API if you ever want).

* [ ] **2.1** Define core RAG types in `genie-core::rag`:

  ```rust
  struct RagCollection {
      id: String,
      name: String,
      description: Option<String>,
      created_at: DateTime<Utc>,
  }

  struct DocumentMeta {
      id: String,
      collection_id: String,
      path: PathBuf,
      title: Option<String>,
  }

  struct Chunk {
      id: String,
      document_id: String,
      collection_id: String,
      text: String,
      embedding: Vec<f32>,
      order: i32,
  }
  ```

* [ ] **2.2** Storage backend:

  * [ ] Use SQLite for metadata & chunk storage.
  * [ ] Table design:

    * `rag_collections`
    * `rag_documents`
    * `rag_chunks`

* [ ] **2.3** Embeddings provider trait:

  ```rust
  trait EmbeddingsProvider: Send + Sync {
      fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingsError>;
  }
  ```

* [ ] **2.4** Default embedding strategy:

  * [ ] For now, make it pluggable & simple:

    * Option 1: Shell out to a separate Python process with a local model (document in README).
    * Option 2: Use an optional external API only if user configures a key.
  * [ ] Encapsulate this complexity behind `EmbeddingsProvider` so Genie itself doesn’t depend on Python directly; but provide a default provider that can be disabled if unavailable.

* [ ] **2.5** Vector search:

  * [ ] Implement cosine similarity in Rust for small-scale usage.
  * [ ] For each query embedding:

    * Compute similarity against all chunks in collection (or a batch).
    * Return top-K chunk IDs.

#### Acceptance Criteria

* [ ] `rag_collections`, `rag_documents`, `rag_chunks` tables exist and can store/retrieve data.
* [ ] Calling `EmbeddingsProvider::embed` on sample text produces deterministic vectors with expected dimensions.
* [ ] A test can insert a few chunks and retrieve top-K via cosine similarity, with the most similar chunk scoring highest.

#### Auto-Testing Prompt

```text
You are an automated tester for Genie's RAG core.

1. Initialize a fresh RAG DB.
2. Create a collection "test-collection".
3. Insert 2 documents with chunks:
   - Chunk A: "Rust is a systems programming language."
   - Chunk B: "Bananas are yellow fruits."

4. Embed and store both chunks.
5. Run a similarity query with text: "Programming in Rust".
6. Verify the top result is Chunk A.

Return JSON:
- `collections_created`: number
- `chunks_inserted`: number
- `top_result_text`: string
- `correct_top_match`: bool
```

---

### 3. CLI: `genie rag` Commands

**Goal:** User-friendly CLI for managing RAG collections and querying them.

#### 3.1 `genie rag init <collection_name>`

* [ ] Create a new `RagCollection` with:

  * `id` (slug or UUID).
  * `name` from CLI.
  * Optional description `--description`.
* [ ] Print collection id and info.

#### 3.2 `genie rag ls`

* [ ] List existing collections with:

  * `id`, `name`, `doc_count`, `chunk_count`.

#### 3.3 `genie rag ingest <collection_id> <path> [options]`

* [ ] Options:

  * `--pattern <glob>` to match files.
  * `--type pdf|text|auto` to control parsing.
  * `--chunk-size <n>` approximate chunk size in tokens/characters.
* [ ] Pipeline:

  * [ ] Recursively walk `<path>` for files matching pattern.
  * [ ] For PDFs: reuse Phase 2 PDF extraction and treat each page/section as text segments.
  * [ ] For text files: read contents directly.
  * [ ] Chunk documents into small pieces (e.g., ~512-1024 tokens).
  * [ ] Call `EmbeddingsProvider` on batches of text.
  * [ ] Store `DocumentMeta` and `Chunk`s in DB.

#### 3.4 `genie rag query <collection_id> "question"`

* [ ] Steps:

  * [ ] Embed the question.
  * [ ] Run similarity search to get top-K chunks (`--top-k`).
  * [ ] Build a prompt including:

    * The question.
    * Retrieved chunk texts as context.
  * [ ] Use `GeminiClient` to generate an answer.
* [ ] Options:

  * `--top-k <n>`
  * `--show-sources` to print chunk metadata used.

#### 3.5 `genie rag rm <collection_id>`

* [ ] Remove collection and all associated documents/chunks (with confirmation).

#### Acceptance Criteria

* [ ] `genie rag init test-collection` creates a new collection visible to `rag ls`.
* [ ] `genie rag ingest <id> ./sample_docs` ingests some files and reports stats.
* [ ] `genie rag query <id> "question"` returns a coherent answer and, if `--show-sources` is used, lists at least one relevant chunk path/snippet.
* [ ] Removing collection cleans up DB entries.

#### Auto-Testing Prompt

```text
You are an automated CLI tester for Genie's `rag` commands.

1. Run: `genie rag init "test-collection" --description "RAG test collection"`.
2. Confirm the new collection appears in `genie rag ls`.
3. Create a test folder with at least one `.txt` file about Rust programming.
4. Run: `genie rag ingest <collection_id> ./test_folder --type text`.
5. Run: `genie rag query <collection_id> "What is Rust used for?" --top-k 3 --show-sources`.
6. Verify:
   - Ingest reports at least one document and multiple chunks.
   - Query returns a non-empty answer and lists the test file as a source.

Return JSON:
- `collection_created`: bool
- `documents_ingested`: number
- `chunks_stored`: number
- `query_answer_excerpt`: string
- `sources_listed`: list of source identifiers.
```

---

### 4. HTTP Endpoints for RAG

**Goal:** Make RAG accessible via HTTP for external apps.

* [ ] **4.1** Add `POST /v1/rag/ingest`:

  * Body: JSON:

    * `collection_id`
    * `path` (server-side path; trust local usage)
    * `pattern`, `type`, `chunk_size` (optional).
  * Response: stats about number of documents/chunks ingested.

* [ ] **4.2** Add `POST /v1/rag/query`:

  * Body:

    * `collection_id`
    * `question`
    * Optional: `top_k`, `return_sources`.
  * Response:

    * `answer: String`
    * Optional `sources: [ { document_path, chunk_text, score } ]`.

* [ ] **4.3** Add `GET /v1/rag/collections`:

  * List collections and basic stats.

#### Acceptance Criteria

* [ ] `/v1/rag/collections` returns a list of known collections.
* [ ] `/v1/rag/ingest` can start a new ingest run and returns stats.
* [ ] `/v1/rag/query` returns answer + sources.
* [ ] HTTP errors are well-formed if collection not found / path invalid.

#### Auto-Testing Prompt

```text
You are an automated HTTP tester for Genie's RAG endpoints.

1. Ensure Genie server is running.
2. Create a collection via CLI or direct DB call and note `collection_id`.
3. Call `POST /v1/rag/ingest` with that `collection_id` and a `path` to a folder with at least one text file.
4. Verify ingest response contains counts: `documents_ingested`, `chunks_ingested`.
5. Call `POST /v1/rag/query` with a relevant question.
6. Verify response has:
   - Non-empty `answer`.
   - Non-empty `sources` array when `return_sources` is true.

Return JSON:
- `collections_listed`: number (from GET /v1/rag/collections)
- `ingest_docs`: number
- `ingest_chunks`: number
- `answer_excerpt`: string
- `sources_count`: number
```

---

### 5. LangChain Integration Examples

**Goal:** Demonstrate how to use Genie as an LLM backend with LangChain.

* [ ] **5.1** Add example Python scripts under `examples/langchain/`.

* [ ] **5.2** Example 1: Basic chat

  ```python
  from langchain_openai import ChatOpenAI

  llm = ChatOpenAI(
      base_url="http://localhost:11435/v1",
      api_key="fake-key",
      model="gemini-2.5-pro"
  )

  resp = llm.invoke("Say hello from Genie")
  print(resp.content)
  ```

* [ ] **5.3** Example 2: RAG chain

  * [ ] Use `ChatOpenAI` with Genie.
  * [ ] Implement LangChain `VectorStoreRetriever` that uses Genie's RAG HTTP endpoints for retrieval.
  * [ ] Simple chain:

    * Query → retriever (calls Genie `/v1/rag/query` for relevant chunks only) → combine with LLM.

* [ ] **5.4** Document how to:

  * [ ] Start Genie server (`genie up`).
  * [ ] Configure `base_url` and `model` in LangChain.
  * [ ] Optionally use streaming.

#### Acceptance Criteria

* [ ] Running example 1 prints a valid response via LangChain.
* [ ] Running example 2 prints an answer that reflects ingested documents in Genie RAG.
* [ ] README or `examples/langchain/README.md` explains steps clearly and they work on fresh environment.

#### Auto-Testing Prompt

```text
You are an automated tester for LangChain integration.

1. Start Genie server.
2. From `examples/langchain`, run:
   - `python basic_chat.py`
   - Capture stdout and ensure it contains non-empty LLM output.

3. Prepare a RAG collection in Genie and configure `rag_chain.py` example to reference it.
4. Run: `python rag_chain.py "Test question"` and verify:
   - The output answer references the ingested content (approximate check based on known file content).

Return JSON:
- `basic_chat_output_excerpt`: string
- `basic_chat_passed`: bool
- `rag_chain_output_excerpt`: string
- `rag_chain_passed`: bool
```

---

### 6. Documentation & Sample Apps

**Goal:** Make advanced features understandable and attractive.

* [ ] **6.1** Update main README:

  * [ ] Add a “RAG & LangChain” section describing what’s possible.
* [ ] **6.2** Create `docs/RAG.md`:

  * [ ] Explain rag concepts and design (collections, documents, chunks).
  * [ ] Step-by-step:

    * `rag init`
    * `rag ingest`
    * `rag query`.
* [ ] **6.3** Create `docs/LANGCHAIN.md`:

  * [ ] Show minimal LangChain config using Genie as backend.
  * [ ] Mention streaming and RAG example.
* [ ] **6.4** Add at least one end-to-end “playbook,” e.g.:

  * [ ] “Turn a folder of project docs into a question-answering system using Genie + LangChain”.

#### Acceptance Criteria

* [ ] Documentation is consistent with CLI and API names.
* [ ] New users can follow docs to build:

  * A simple chat app using Genie + LangChain.
  * A basic RAG-backed Q&A over some local docs.
* [ ] No references to non-existent commands/endpoints.

#### Auto-Testing Prompt

```text
You are an automated documentation consistency checker.

1. Parse `docs/RAG.md` and `docs/LANGCHAIN.md` to extract command names and HTTP endpoints.
2. Compare them against the actual CLI and server routes in the codebase.
3. Verify:
   - All referenced commands (`genie rag ...`) exist.
   - All referenced HTTP paths (`/v1/rag/...`, `/v1/chat/completions`) are implemented.
4. Identify any mismatches or missing documentation.

Return JSON:
- `documented_commands`: list
- `implemented_but_undocumented_commands`: list
- `referenced_but_missing_commands`: list
- `docs_consistent`: bool
```

---

### 7. Hardening & Performance Considerations

**Goal:** Ensure RAG and streaming don’t degrade core performance.

* [ ] **7.1** Add basic rate limiting for RAG HTTP endpoints (optional).
* [ ] **7.2** Add configuration for:

  * Maximum chunks per collection (prevent DB explosion).
  * Maximum concurrent RAG queries.
* [ ] **7.3** Profiling:

  * [ ] Use simple benchmarks to measure:

    * Embedding throughput.
    * Query latency for N chunks.
* [ ] **7.4** Add config flags to disable RAG or embeddings provider if unavailable (fallback gracefully).

#### Acceptance Criteria

* [ ] With RAG enabled, normal chat endpoints still function with acceptable latency.
* [ ] If embeddings provider is disabled/misconfigured, RAG commands fail with clear error messages.
* [ ] Collections with many chunks still can be queried within reasonable time for small demos.

#### Auto-Testing Prompt

```text
You are an automated performance sanity tester.

1. Ingest enough documents to create at least 1000 chunks in a RAG collection.
2. Measure the time taken to respond to 10 consecutive `/v1/rag/query` requests.
3. Measure the time taken to respond to 10 consecutive `/v1/chat/completions` requests without RAG.
4. Compare average latencies and ensure:
   - RAG queries are under a reasonable threshold (e.g., < 2 seconds for small collections).
   - Chat completions remain under a similar threshold as Phase 1 baseline.

Return JSON:
- `avg_rag_latency_ms`: number
- `avg_chat_latency_ms`: number
- `rag_enabled`: bool
- `performance_ok`: bool (based on thresholds).
```

```
::contentReference[oaicite:0]{index=0}
```
