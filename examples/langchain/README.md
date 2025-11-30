# LangChain + Genie Examples

This directory contains examples of using [LangChain](https://www.langchain.com/) with Genie as an OpenAI-compatible backend.

## Prerequisites

1. **Install Python dependencies:**
   ```bash
   pip install langchain-openai requests
   ```

2. **Start Genie server:**
   ```bash
   genie up
   ```
   
   Or run in daemon mode:
   ```bash
   genie up --daemon
   ```

## Examples

### 1. Basic Chat (`basic_chat.py`)

Demonstrates basic LangChain usage with Genie:
- Simple message invocation
- System prompts
- Multi-turn conversations

```bash
python basic_chat.py
```

### 2. RAG Chain (`rag_chain.py`)

Demonstrates RAG (Retrieval-Augmented Generation) with Genie:
- Using Genie's built-in RAG endpoint
- Using LangChain with custom retriever

**Setup:**
```bash
# Create a RAG collection
genie rag init my-docs --description "My document collection"

# Ingest some documents
genie rag ingest my-docs /path/to/your/documents

# Run the example
python rag_chain.py "What is this project about?" my-docs
```

## Configuration

By default, examples connect to Genie at `http://localhost:11435`. 
To use a different URL, modify the `GENIE_URL` or `base_url` variables.

## How It Works

Genie provides an OpenAI-compatible API at `/v1/chat/completions`. This means you can use LangChain's `ChatOpenAI` class by simply pointing it to Genie:

```python
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(
    base_url="http://localhost:11435/v1",
    api_key="genie-local",  # Any string works
    model="gemini-2.5-pro",
)

response = llm.invoke("Hello!")
print(response.content)
```

## Available Models

Genie exposes the following models (configurable in `~/.genie/config.toml`):
- `gemini-2.5-pro` (default)
- `gemini-2.5-flash`

## Streaming (Coming Soon)

Streaming support is available via the `stream=True` parameter:

```python
for chunk in llm.stream("Tell me a story"):
    print(chunk.content, end="", flush=True)
```

## Troubleshooting

### Connection Refused
Make sure Genie is running: `genie status`

### Rate Limiting
Check your quota: `genie quota status`

### No Collections Found (RAG)
Create a collection first:
```bash
genie rag init my-collection
genie rag ingest my-collection /path/to/docs
```

