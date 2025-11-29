# Genie Usage Guide

This guide covers all Genie commands and features in detail.

## Table of Contents

- [Basic Commands](#basic-commands)
- [PDF Summarization](#pdf-summarization)
- [Book Summarization](#book-summarization)
- [Repository Summarization](#repository-summarization)
- [Prompt Templates](#prompt-templates)
- [Server & HTTP API](#server--http-api)
- [Quota Management](#quota-management)
- [Configuration](#configuration)

---

## Basic Commands

### `genie ask`

Send a one-shot prompt to Gemini.

```bash
# Basic usage
genie ask "Explain quantum computing in simple terms"

# Read prompt from stdin
echo "What is Rust?" | genie ask

# Use a specific model
genie --model gemini-2.5-flash ask "Hello"

# Ignore quota limits
genie --ignore-quota ask "Important prompt"
```

### `genie json`

Send a prompt and get a guaranteed JSON response.

```bash
# Basic JSON request
genie json "Return a JSON object with fields: name, age, city"

# With schema validation (coming soon)
genie json "Generate user data" --schema user_schema.json
```

---

## PDF Summarization

### `genie summarize-pdf`

Summarize a PDF document into structured notes.

```bash
# Basic usage
genie summarize-pdf document.pdf

# Specify output format
genie summarize-pdf document.pdf --format json
genie summarize-pdf document.pdf --format markdown
genie summarize-pdf document.pdf --format both

# Choose summary style
genie summarize-pdf document.pdf --style concise      # Brief summary
genie summarize-pdf document.pdf --style detailed     # In-depth analysis
genie summarize-pdf document.pdf --style exam-notes   # Study-focused
genie summarize-pdf document.pdf --style bullet       # Bullet points only

# Output to specific file
genie summarize-pdf document.pdf --out my_summary

# Specify language
genie summarize-pdf document.pdf --language es  # Spanish output
genie summarize-pdf document.pdf --language de  # German output
```

**Output:**
- `<filename>_summary.json` - Structured JSON with summary, key points
- `<filename>_summary.md` - Human-readable Markdown

---

## Book Summarization

### `genie summarize-book`

Summarize a book with automatic chapter detection.

```bash
# Basic usage
genie summarize-book textbook.pdf

# Full example with all options
genie summarize-book textbook.pdf \
  --style detailed \
  --language en \
  --format both \
  --out my_book_summary
```

**Features:**
- Automatic chapter detection using font heuristics
- Per-chapter summaries with key points
- Global summary and reading roadmap
- Important terms extraction
- Reflection questions for each chapter

**Output Structure (JSON):**
```json
{
  "title": "Book Title",
  "author": "Author Name",
  "chapters": [
    {
      "chapter_id": 1,
      "title": "Chapter 1: Introduction",
      "summary": "This chapter covers...",
      "key_points": ["Point 1", "Point 2"],
      "important_terms": ["Term 1", "Term 2"],
      "questions_for_reflection": ["Question 1"]
    }
  ],
  "global_summary": "Overall book summary...",
  "reading_roadmap": ["Start with Ch 1...", "Then read Ch 3..."],
  "page_count": 250
}
```

---

## Repository Summarization

### `genie repo-summary`

Analyze and summarize a code repository.

```bash
# Summarize current directory
genie repo-summary

# Summarize specific path
genie repo-summary /path/to/repo

# Output as JSON
genie repo-summary --format json

# Limit files for quick summary
genie repo-summary --max-files 50

# Save to file
genie repo-summary --out summary.md
```

**Features:**
- Respects `.gitignore`
- Detects 30+ programming languages
- Groups files by module/directory
- Identifies key files and technologies
- Provides architecture overview

**Output Structure (JSON):**
```json
{
  "name": "my-project",
  "overview": "This is a Rust web application that...",
  "modules": [
    {
      "path": "src/api",
      "description": "HTTP API handlers and routing",
      "key_files": ["routes.rs", "handlers.rs"],
      "technologies": ["axum", "tokio"]
    }
  ],
  "languages": ["Rust", "TypeScript"],
  "file_count": 45,
  "total_lines": 5200
}
```

---

## Prompt Templates

Templates allow you to create reusable prompts with variables.

### Template Format

Templates are stored in `~/.genie/prompts/*.prompt.md`:

```yaml
---
name: "code-review"
description: "Review code for bugs and improvements"
model: "gemini-2.5-pro"
input_variables:
  - name: "language"
    description: "Programming language"
    default: "auto-detect"
  - name: "focus"
    description: "Review focus areas"
    default: "bugs, performance"
  - name: "file"
    type: "file"
    description: "Code file to review"
    required: true
json_output: true
---
Review this {{ language }} code for {{ focus }}.

Code:
{{ file_content }}

Return JSON with issues found.
```

### Variable Types

- `string` (default): Text input
- `file`: File path, content available as `{{ name_content }}` or `{{ file_content }}`
- `number`: Numeric input
- `boolean`: true/false
- `enum`: Choice from list

### Template Commands

```bash
# List all templates
genie templates list

# Show template details
genie templates show code-review

# Run a template
genie templates run code-review \
  --var language=rust \
  --var focus="security,performance" \
  --file file=src/main.rs

# Create a new template
genie templates new my-template
```

### Example: Code Review Template

Create `~/.genie/prompts/review.prompt.md`:

```yaml
---
name: "review"
description: "Quick code review"
input_variables:
  - name: "file"
    type: "file"
    required: true
json_output: true
---
Review this code briefly. Return JSON:
{"issues": [], "suggestions": [], "score": 1-10}

{{ file_content }}
```

Run it:
```bash
genie templates run review --file file=app.py
```

---

## Server & HTTP API

### Starting the Server

```bash
# Start with TUI dashboard
genie up

# Start as background daemon
genie up --daemon

# Custom port
genie --port 8080 up
```

### TUI Controls

- `q` - Quit
- `Space` - Toggle view mode
- View shows: quota usage, recent requests, server status

### HTTP Endpoints

Base URL: `http://localhost:11435`

#### Chat Completions (OpenAI-compatible)

```bash
curl -X POST http://localhost:11435/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gemini-2.5-pro",
    "messages": [
      {"role": "system", "content": "You are helpful"},
      {"role": "user", "content": "Hello!"}
    ],
    "temperature": 0.7,
    "max_tokens": 1000
  }'
```

#### JSON Completions

```bash
curl -X POST http://localhost:11435/v1/json \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gemini-2.5-pro",
    "messages": [
      {"role": "user", "content": "Return a JSON object with name and age"}
    ]
  }'
```

#### Document Summarization

```bash
curl -X POST http://localhost:11435/v1/docs/summarize \
  -H "Content-Type: application/json" \
  -d '{
    "path": "/path/to/document.pdf",
    "mode": "pdf",
    "style": "concise",
    "language": "en"
  }'
```

Options:
- `mode`: `"pdf"` or `"book"`
- `style`: `"concise"`, `"detailed"`, `"exam-notes"`, `"bullet"`
- `language`: Language code (e.g., `"en"`, `"es"`, `"de"`)

#### Repository Summarization

```bash
curl -X POST http://localhost:11435/v1/repo/summary \
  -H "Content-Type: application/json" \
  -d '{
    "path": "/path/to/repo",
    "max_files": 100
  }'
```

#### Quota Status

```bash
curl http://localhost:11435/v1/quota
```

Response:
```json
{
  "requests_today": 42,
  "requests_per_day_limit": 1000,
  "requests_last_minute": 3,
  "requests_per_minute_limit": 60,
  "approx_input_tokens_today": 15000,
  "approx_output_tokens_today": 8000,
  "reset_time": "00:00"
}
```

#### Health Check

```bash
curl http://localhost:11435/health
```

---

## Quota Management

### Check Status

```bash
# Human-readable output
genie quota status

# JSON output
genie quota status --json
```

### View Usage Log

```bash
# Last 10 requests
genie quota log

# Last 50 requests
genie quota log --last 50
```

### Bypass Quota

```bash
# Use with caution!
genie --ignore-quota ask "Important prompt"
```

---

## Configuration

### Config File

Location: `~/.genie/config.toml`

```toml
[gemini]
binary = "gemini"
default_model = "gemini-2.5-pro"

[server]
host = "127.0.0.1"
port = 11435

[quota]
per_minute = 60
per_day = 1000
reset_time = "00:00"

[logging]
level = "info"
```

### Initialize Config

```bash
# Create default config
genie config init

# Overwrite existing
genie config init --force
```

### Show Current Config

```bash
genie config show
```

### Environment Variables

- `GENIE_MODEL` - Default model
- `GENIE_PORT` - Server port
- `GENIE_HOST` - Server host
- `GENIE_LOG_LEVEL` - Log level (trace, debug, info, warn, error)

### CLI Overrides

```bash
genie --model gemini-2.5-flash ask "Hello"
genie --port 8080 up
genie --log-level debug ask "Debug me"
```

---

## Examples

### Summarize a Research Paper

```bash
genie summarize-pdf paper.pdf \
  --style detailed \
  --format both \
  --language en
```

### Summarize a Textbook for Study

```bash
genie summarize-book textbook.pdf \
  --style exam-notes \
  --out study_guide
```

### Analyze Your Codebase

```bash
cd my-project
genie repo-summary --format json --out architecture.json
```

### Create a Custom Review Template

```bash
genie templates new my-review
# Edit ~/.genie/prompts/my-review.prompt.md
genie templates run my-review --file file=code.rs
```

### Use Genie with LangChain

```python
from langchain.llms import OpenAI

llm = OpenAI(
    base_url="http://localhost:11435/v1",
    api_key="not-needed"  # Genie doesn't require keys
)

response = llm("Explain recursion")
```

---

## Troubleshooting

### Gemini CLI Not Found

```bash
# Install Gemini CLI
npm install -g @google/gemini-cli

# Authenticate
gemini
```

### Quota Exceeded

```bash
# Check status
genie quota status

# View what's using quota
genie quota log --last 20

# Temporary override (use sparingly)
genie --ignore-quota ask "Urgent"
```

### Server Already Running

```bash
# Check status
genie status

# Stop existing server
genie stop

# Start fresh
genie up
```

