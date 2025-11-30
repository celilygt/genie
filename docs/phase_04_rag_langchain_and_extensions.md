
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

- **M1:** OpenAI API compatibility enhancements (streaming, extra params). ✅
- **M2:** RAG data model & vector store integration. ✅
- **M3:** CLI commands: `rag init`, `rag ingest`, `rag query`, `rag ls`. ✅
- **M4:** HTTP endpoints for RAG. ✅
- **M5:** LangChain integration examples (Python). ✅
- **M6:** Documentation & sample apps. ✅

---

## Task Breakdown

### 1. OpenAI API Compatibility Enhancements

**Goal:** Make `/v1/chat/completions` closer to the OpenAI spec + support streaming.

- [x] **1.1** Extend `ChatCompletionRequest` model:
  - [x] Support optional fields:
    - `top_p`, `presence_penalty`, `frequency_penalty` (even if just stored for future).
    - `stop`, `n`, `user` (may be ignored or lightly used).
- [x] **1.2** Implement `stream: true` support:
  - [x] Define streaming chunk types (`ChatCompletionChunk`, `ChunkChoice`, `ChunkDelta`)
  - [x] Add `to_sse()` method for SSE formatting
  - [ ] Full streaming implementation with `gemini` CLI streaming mode (deferred)
- [x] **1.3** Error shapes:
  - [x] Wrap errors into JSON matching `{"error": {"message": "...", "type": "...", ...}}`.

- [x] **1.4** Compatible headers & behavior:
  - [x] Accept `Authorization: Bearer <fake-key>` for LangChain (not enforced).
  - [x] Ensure CORS if needed (for browser-based clients).

#### Acceptance Criteria

- [x] LangChain's `ChatOpenAI` can talk to Genie using basic chat.
- [x] Unsupported fields are safely ignored with no crashes.
- [x] Errors from Genie are surfaced as proper OpenAI-like error objects.

---

### 2. RAG Data Model & Vector Store

**Goal:** Design and implement a simple, local RAG subsystem.

* [x] **2.1** Define core RAG types in `genie-core::rag`:

  - `RagCollection`, `DocumentMeta`, `Chunk`, `QueryResult`, `RagQueryResponse`

* [x] **2.2** Storage backend:

  * [x] Use SQLite for metadata & chunk storage.
  * [x] Table design: `rag_collections`, `rag_documents`, `rag_chunks`

* [x] **2.3** Simple text-based similarity search:

  * [x] Implement keyword-based search as initial approach
  * [x] Implement `cosine_similarity` function for future embedding support

* [x] **2.4** Vector search preparation:

  * [x] Implement cosine similarity in Rust for small-scale usage.
  * [x] Store embedding field in Chunk struct (for future use)

#### Acceptance Criteria

* [x] `rag_collections`, `rag_documents`, `rag_chunks` tables exist and can store/retrieve data.
* [x] Simple text search can find relevant chunks.
* [x] A test can insert a few chunks and retrieve top-K via text matching.

---

### 3. CLI: `genie rag` Commands

**Goal:** User-friendly CLI for managing RAG collections and querying them.

#### 3.1 `genie rag init <collection_name>`

* [x] Create a new `RagCollection` with:
  * `id` (UUID).
  * `name` from CLI.
  * Optional description `--description`.
* [x] Print collection id and info.

#### 3.2 `genie rag ls`

* [x] List existing collections with:
  * `id`, `name`, `doc_count`, `chunk_count`.

#### 3.3 `genie rag ingest <collection_id> <path> [options]`

* [x] Options:
  * `--pattern <glob>` to match files.
  * `--chunk-size <n>` approximate chunk size in characters.
* [x] Pipeline:
  * [x] Recursively walk `<path>` for files matching pattern.
  * [x] For PDFs: reuse Phase 2 PDF extraction.
  * [x] For text files: read contents directly.
  * [x] Chunk documents into small pieces.
  * [x] Store `DocumentMeta` and `Chunk`s in DB.

#### 3.4 `genie rag query <collection_id> "question"`

* [x] Steps:
  * [x] Run similarity search to get top-K chunks (`--top-k`).
  * [x] Build a prompt including retrieved chunk texts as context.
  * [x] Use `GeminiClient` to generate an answer.
* [x] Options:
  * `--top-k <n>`
  * `--show-sources` to print chunk metadata used.

#### 3.5 `genie rag rm <collection_id>`

* [x] Remove collection and all associated documents/chunks (with `--force` confirmation).

#### Acceptance Criteria

* [x] `genie rag init test-collection` creates a new collection visible to `rag ls`.
* [x] `genie rag ingest <id> ./sample_docs` ingests some files and reports stats.
* [x] `genie rag query <id> "question"` returns a coherent answer.
* [x] Removing collection cleans up DB entries.

---

### 4. HTTP Endpoints for RAG

**Goal:** Make RAG accessible via HTTP for external apps.

* [x] **4.1** Add `POST /v1/rag/ingest`:
  * Body: JSON: `collection_id`, `path`, `pattern`, `chunk_size` (optional).
  * Response: stats about number of documents/chunks ingested.

* [x] **4.2** Add `POST /v1/rag/query`:
  * Body: `collection_id`, `question`, `top_k`, `return_sources`.
  * Response: `answer`, `sources` array.

* [x] **4.3** Add `GET /v1/rag/collections`:
  * List collections and basic stats.

#### Acceptance Criteria

* [x] `/v1/rag/collections` returns a list of known collections.
* [x] `/v1/rag/ingest` can start a new ingest run and returns stats.
* [x] `/v1/rag/query` returns answer + sources.
* [x] HTTP errors are well-formed if collection not found / path invalid.

---

### 5. LangChain Integration Examples

**Goal:** Demonstrate how to use Genie as an LLM backend with LangChain.

* [x] **5.1** Add example Python scripts under `examples/langchain/`.

* [x] **5.2** Example 1: Basic chat (`basic_chat.py`)
  - Simple message invocation
  - System prompts
  - Multi-turn conversations

* [x] **5.3** Example 2: RAG chain (`rag_chain.py`)
  * [x] Use `ChatOpenAI` with Genie.
  * [x] Custom `GenieRetriever` that uses Genie's RAG HTTP endpoints.
  * [x] Two approaches: direct Genie RAG endpoint, LangChain with custom retriever.

* [x] **5.4** Document how to:
  * [x] Start Genie server (`genie up`).
  * [x] Configure `base_url` and `model` in LangChain.
  * [x] `examples/langchain/README.md` with setup instructions.

#### Acceptance Criteria

* [x] Running example 1 prints a valid response via LangChain.
* [x] Running example 2 prints an answer that reflects ingested documents.
* [x] README explains steps clearly.

---

### 6. Documentation & Sample Apps

**Goal:** Make advanced features understandable and attractive.

* [x] **6.1** Update examples with LangChain README.
* [ ] **6.2** Create `docs/RAG.md` (pending).
* [ ] **6.3** Create `docs/LANGCHAIN.md` (pending).
* [ ] **6.4** Add end-to-end playbook (pending).

---

### 7. Hardening & Performance Considerations

**Goal:** Ensure RAG and streaming don't degrade core performance.

* [ ] **7.1** Add basic rate limiting for RAG HTTP endpoints (optional).
* [ ] **7.2** Add configuration for maximum chunks per collection.
* [ ] **7.3** Profiling (deferred).
* [ ] **7.4** Add config flags to disable RAG if unavailable.

---

## Implementation Summary

### Completed Features

1. **OpenAI API Compatibility**
   - Extended `ChatCompletionRequest` with optional fields: `top_p`, `presence_penalty`, `frequency_penalty`, `stop`, `n`, `user`
   - Added streaming types: `ChatCompletionChunk`, `ChunkChoice`, `ChunkDelta`
   - CORS enabled for browser-based clients

2. **RAG Module (`genie-core/src/rag.rs`)**
   - SQLite storage for collections, documents, and chunks
   - Text-based similarity search (keyword matching)
   - Cosine similarity function for future embedding support
   - PDF and text file ingestion
   - Chunking with configurable size

3. **CLI Commands**
   - `genie rag init <name>` - Create collection
   - `genie rag ls` - List collections
   - `genie rag ingest <collection> <path>` - Ingest documents
   - `genie rag query <collection> "question"` - Query with RAG
   - `genie rag rm <collection>` - Delete collection

4. **HTTP Endpoints**
   - `GET /v1/rag/collections` - List collections
   - `POST /v1/rag/ingest` - Ingest documents
   - `POST /v1/rag/query` - Query with RAG

5. **LangChain Examples**
   - `examples/langchain/basic_chat.py` - Basic chat example
   - `examples/langchain/rag_chain.py` - RAG chain example
   - `examples/langchain/README.md` - Setup documentation

```
