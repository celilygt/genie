#!/usr/bin/env python3
"""
RAG Chain Example using Genie as Backend

This example demonstrates how to use Genie's RAG capabilities with LangChain.
It shows two approaches:
1. Using Genie's built-in RAG endpoint
2. Using LangChain's retrieval chain with Genie as LLM

Prerequisites:
1. Start Genie server: `genie up`
2. Create and populate a RAG collection:
   genie rag init my-docs
   genie rag ingest my-docs /path/to/your/docs
3. Install dependencies: `pip install langchain-openai requests`

Usage:
    python rag_chain.py "Your question here" [collection_name]
"""

import sys
import json
import requests
from typing import List, Dict, Any
from langchain_openai import ChatOpenAI
from langchain_core.messages import SystemMessage, HumanMessage

# Genie server URL
GENIE_URL = "http://localhost:11435"


class GenieRetriever:
    """Custom retriever that uses Genie's RAG endpoint."""

    def __init__(self, collection_id: str, top_k: int = 5):
        self.collection_id = collection_id
        self.top_k = top_k

    def retrieve(self, query: str) -> List[Dict[str, Any]]:
        """Retrieve relevant documents from Genie RAG."""
        response = requests.post(
            f"{GENIE_URL}/v1/rag/query",
            json={
                "collection_id": self.collection_id,
                "question": query,
                "top_k": self.top_k,
                "return_sources": True,
            },
        )

        if response.status_code != 200:
            raise Exception(f"RAG query failed: {response.text}")

        data = response.json()
        return data.get("sources", [])


def use_genie_rag_endpoint(question: str, collection_id: str):
    """
    Approach 1: Use Genie's built-in RAG endpoint directly.
    This is the simplest approach - Genie handles retrieval and LLM call.
    """
    print("\n" + "=" * 60)
    print("Approach 1: Using Genie's Built-in RAG Endpoint")
    print("=" * 60)

    response = requests.post(
        f"{GENIE_URL}/v1/rag/query",
        json={
            "collection_id": collection_id,
            "question": question,
            "top_k": 5,
            "return_sources": True,
        },
    )

    if response.status_code != 200:
        print(f"Error: {response.text}")
        return

    data = response.json()
    print(f"\nQuestion: {question}")
    print(f"\nAnswer: {data['answer']}")

    if data.get("sources"):
        print("\nSources:")
        for i, source in enumerate(data["sources"], 1):
            print(f"  {i}. {source['document_path']} (score: {source['score']:.2f})")


def use_langchain_with_genie(question: str, collection_id: str):
    """
    Approach 2: Use LangChain with Genie as both retriever and LLM.
    More flexible - allows custom prompting and chain composition.
    """
    print("\n" + "=" * 60)
    print("Approach 2: Using LangChain with Genie Backend")
    print("=" * 60)

    # Initialize Genie-backed LLM
    llm = ChatOpenAI(
        base_url=f"{GENIE_URL}/v1",
        api_key="genie-local",
        model="gemini-2.5-pro",
        temperature=0.3,  # Lower temperature for RAG
    )

    # Initialize custom retriever
    retriever = GenieRetriever(collection_id, top_k=5)

    # Retrieve relevant documents
    print(f"\nQuestion: {question}")
    print("\nRetrieving relevant documents...")

    sources = retriever.retrieve(question)

    if not sources:
        print("No relevant documents found.")
        return

    print(f"Found {len(sources)} relevant chunks")

    # Build context from retrieved documents
    context_parts = []
    for source in sources:
        chunk_text = source.get("chunk", {}).get("text", "")
        doc_path = source.get("document_path", "unknown")
        context_parts.append(f"[Source: {doc_path}]\n{chunk_text}")

    context = "\n\n---\n\n".join(context_parts)

    # Build RAG prompt
    system_prompt = """You are a helpful assistant that answers questions based on the provided context.
If the answer is not in the context, say so clearly.
Always cite which source(s) you used to answer the question."""

    user_prompt = f"""Context:
{context}

Question: {question}

Please provide a detailed answer based on the context above."""

    # Call LLM with RAG context
    messages = [
        SystemMessage(content=system_prompt),
        HumanMessage(content=user_prompt),
    ]

    print("\nGenerating answer...")
    response = llm.invoke(messages)

    print(f"\nAnswer: {response.content}")

    print("\nSources used:")
    for i, source in enumerate(sources, 1):
        doc_path = source.get("document_path", "unknown")
        score = source.get("score", 0)
        print(f"  {i}. {doc_path} (relevance: {score:.2f})")


def list_collections():
    """List available RAG collections."""
    response = requests.get(f"{GENIE_URL}/v1/rag/collections")
    if response.status_code != 200:
        print(f"Error: {response.text}")
        return []

    collections = response.json()
    return collections


def main():
    if len(sys.argv) < 2:
        print("Usage: python rag_chain.py <question> [collection_name]")
        print("\nAvailable collections:")

        collections = list_collections()
        if collections:
            for col in collections:
                print(f"  - {col['name']} ({col['document_count']} docs, {col['chunk_count']} chunks)")
        else:
            print("  No collections found. Create one with:")
            print("    genie rag init <name>")
            print("    genie rag ingest <name> /path/to/docs")
        return

    question = sys.argv[1]
    collection_id = sys.argv[2] if len(sys.argv) > 2 else None

    # If no collection specified, try to use the first available
    if not collection_id:
        collections = list_collections()
        if collections:
            collection_id = collections[0]["name"]
            print(f"Using collection: {collection_id}")
        else:
            print("No collections found. Please create one first:")
            print("  genie rag init <name>")
            print("  genie rag ingest <name> /path/to/docs")
            return

    # Demonstrate both approaches
    use_genie_rag_endpoint(question, collection_id)
    use_langchain_with_genie(question, collection_id)


if __name__ == "__main__":
    main()

