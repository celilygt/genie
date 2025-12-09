# 🧞 Genie

**Local Gemini-as-a-service** - A Rust application that wraps the official `gemini` CLI with quota tracking and power tools.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## ✨ Features

- **🔌 Gemini CLI Wrapper**: Use Google's Gemini AI through a clean interface
- **📊 Quota Tracking**: Monitor your usage with requests/minute and requests/day limits
- **🌐 OpenAI-Compatible API**: Drop-in replacement for OpenAI's `/v1/chat/completions` endpoint
- **📺 TUI Dashboard**: Tilt-like terminal UI for monitoring your Genie daemon
- **📄 PDF/Book Summarization**: Summarize PDFs with automatic chapter detection
- **📁 Repo Summarization**: Analyze and summarize code repositories
- **📝 Prompt Templates**: Reusable prompts with variable interpolation
- **⚡ Async Rust**: Built with Tokio for high performance

## 📋 Prerequisites

Before using Genie, you need to have the Gemini CLI installed and authenticated:

```bash
# Install Gemini CLI
npm install -g @google/gemini-cli

# Authenticate (follow the prompts)
gemini
```

## 🚀 Installation

### From source

```bash
# Clone the repository
git clone https://github.com/celilygt/genie.git
cd genie

# Build and install
cargo install --path genie-cli
```

### Using Cargo

```bash
cargo install genie-cli
```

## 📖 Usage

### Basic Commands

```bash
# Simple prompt
genie ask "Explain quantum computing in simple terms"

# JSON response
genie json "Return a JSON object with name and age fields"

# Check quota status
genie quota status

# View recent usage
genie quota log

# View configuration
genie config show
```

### PDF & Book Summarization

```bash
# Summarize a PDF document
genie summarize-pdf document.pdf --style concise --format both

# Summarize a book with chapter detection
genie summarize-book textbook.pdf --style detailed --language en

# Options:
#   --style: concise, detailed, exam-notes, bullet
#   --format: json, markdown, both
#   --out: output file path
#   --language: output language (default: en)
```

### Repository Summarization

```bash
# Summarize current directory
genie repo-summary

# Summarize specific repo
genie repo-summary /path/to/repo --format json --out summary.json

# Limit files for quick summary
genie repo-summary . --max-files 50
```

### Prompt Templates

Templates are stored in `~/.genie/prompts/*.prompt.md` with YAML frontmatter:

```yaml
---
name: "my-template"
description: "A custom prompt template"
model: "gemini-2.5-pro"
input_variables:
  - name: "topic"
    description: "Topic to write about"
    default: "Rust"
  - name: "file"
    type: "file"
    description: "Input file"
---
Write about {{ topic }}.

Content: {{ file_content }}
```

```bash
# List available templates
genie templates list

# Show template details
genie templates show my-template

# Run a template
genie templates run my-template --var topic=Python --file file=input.txt

# Create new template
genie templates new my-new-template
```

### Start the Daemon (with TUI)

```bash
# Start with terminal UI
genie up

# Or run as background daemon
genie up --daemon
```

The TUI shows:
- Real-time quota usage (daily and per-minute)
- Recent request log
- Server status

Press `q` to quit, `Space` to toggle view modes.

### HTTP API

When the daemon is running, you can use the OpenAI-compatible API:

```bash
curl http://localhost:11435/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gemini-2.5-pro",
    "messages": [
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

Available endpoints:
- `POST /v1/chat/completions` - OpenAI-compatible chat
- `POST /v1/json` - Guaranteed JSON response
- `POST /v1/docs/summarize` - PDF/Book summarization
- `POST /v1/repo/summary` - Repository summarization
- `GET /v1/quota` - Quota status
- `GET /v1/models` - List available models
- `GET /health` - Health check

#### Document Summarization API

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

#### Repo Summary API

```bash
curl -X POST http://localhost:11435/v1/repo/summary \
  -H "Content-Type: application/json" \
  -d '{
    "path": "/path/to/repo",
    "max_files": 100
  }'
```

## ⚙️ Configuration

Configuration file: `~/.genie/config.toml`

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

### Environment Variables

- `GENIE_MODEL` - Override default model
- `GENIE_PORT` - Override server port
- `GENIE_HOST` - Override server host
- `GENIE_LOG_LEVEL` - Set log level

### CLI Overrides

```bash
genie --model gemini-2.5-flash ask "Hello"
genie --port 8080 up
```

## 🏗️ Architecture

```
genie/
├── genie-core/          # Core library
│   ├── config.rs        # Configuration management
│   ├── gemini.rs        # Gemini CLI wrapper
│   ├── quota.rs         # SQLite usage tracking
│   ├── server.rs        # HTTP API (Axum)
│   ├── model.rs         # Shared types
│   ├── pdf.rs           # PDF extraction & chapter detection
│   ├── docs.rs          # Document summarization pipeline
│   ├── repo.rs          # Repository analysis & summarization
│   └── templates.rs     # Prompt template engine
├── genie-cli/           # CLI binary
│   ├── commands/        # CLI command implementations
│   └── tui/             # Terminal UI (ratatui)
└── docs/                # Documentation
```

## 🔒 Quota Limits

Genie tracks and enforces quotas based on Gemini CLI's free tier:
- **60 requests/minute**
- **1,000 requests/day**

Use `--ignore-quota` to bypass limits (at your own risk):

```bash
genie --ignore-quota ask "Important prompt"
```

## 🤝 Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

## 📄 License

MIT License - see [LICENSE](LICENSE) for details.

## 🙏 Acknowledgments

- [Google Gemini CLI](https://github.com/google-gemini/gemini-cli) - The underlying AI interface
- [Ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [Axum](https://github.com/tokio-rs/axum) - Web framework

