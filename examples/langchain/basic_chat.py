#!/usr/bin/env python3
"""
Basic LangChain Chat Example using Genie as Backend

This example demonstrates how to use Genie as an OpenAI-compatible backend
for LangChain's ChatOpenAI class.

Prerequisites:
1. Start Genie server: `genie up`
2. Install dependencies: `pip install langchain-openai`

Usage:
    python basic_chat.py
"""

from langchain_openai import ChatOpenAI


def main():
    # Configure LangChain to use Genie as the backend
    # Note: api_key can be any string since Genie doesn't require authentication
    llm = ChatOpenAI(
        base_url="http://localhost:11435/v1",
        api_key="genie-local",  # Can be any string
        model="gemini-2.5-pro",
        temperature=0.7,
    )

    print("=" * 60)
    print("Genie + LangChain Basic Chat Example")
    print("=" * 60)
    print()

    # Example 1: Simple message
    print("Example 1: Simple greeting")
    print("-" * 40)
    response = llm.invoke("Say hello from Genie!")
    print(f"Response: {response.content}")
    print()

    # Example 2: With system message
    print("Example 2: With system prompt")
    print("-" * 40)
    from langchain_core.messages import SystemMessage, HumanMessage

    messages = [
        SystemMessage(content="You are a helpful coding assistant. Be concise."),
        HumanMessage(content="What is a closure in programming?"),
    ]
    response = llm.invoke(messages)
    print(f"Response: {response.content}")
    print()

    # Example 3: Multi-turn conversation
    print("Example 3: Multi-turn conversation")
    print("-" * 40)
    from langchain_core.messages import AIMessage

    conversation = [
        SystemMessage(content="You are a math tutor. Explain concepts step by step."),
        HumanMessage(content="What is 15% of 80?"),
    ]

    # First turn
    response1 = llm.invoke(conversation)
    print(f"User: What is 15% of 80?")
    print(f"Assistant: {response1.content}")

    # Second turn
    conversation.append(AIMessage(content=response1.content))
    conversation.append(HumanMessage(content="Now calculate 20% of 150"))

    response2 = llm.invoke(conversation)
    print(f"\nUser: Now calculate 20% of 150")
    print(f"Assistant: {response2.content}")

    print()
    print("=" * 60)
    print("All examples completed successfully!")
    print("=" * 60)


if __name__ == "__main__":
    main()

