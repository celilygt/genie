# 🧞 Genie

**Local Gemini-as-a-service** - A Rust application that wraps the official `gemini` CLI with quota tracking and power tools.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## ✨ Features

- **🔌 Gemini CLI Wrapper**: Use Google's Gemini AI through a clean interface
- **📊 Quota Tracking**: Monitor your usage with requests/minute and requests/day limits
- **🌐 OpenAI-Compatible API**: Drop-in replacement for OpenAI's `/v1/chat/completions` endpoint
- **📺 TUI Dashboard**: Tilt-like terminal UI for monitoring your Genie daemon
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
git clone https://github.com/yourusername/genie.git
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
- `GET /v1/quota` - Quota status
- `GET /v1/models` - List available models
- `GET /health` - Health check

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
│   └── model.rs         # Shared types
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

