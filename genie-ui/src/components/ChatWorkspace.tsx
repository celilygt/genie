import { useState, useRef, useEffect } from 'react';
import { MessageSquare, Send, Sparkles } from 'lucide-react';
import { useChat } from '../hooks/useGenie';
import type { ChatMessage } from '../types';

export function ChatWorkspace() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const { sendMessage, isLoading, error } = useChat();

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || isLoading) return;

    const userMessage: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: input.trim(),
      timestamp: new Date(),
    };

    setMessages((prev) => [...prev, userMessage]);
    setInput('');

    const response = await sendMessage({ message: userMessage.content });

    if (response) {
      const assistantMessage: ChatMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: response.message,
        timestamp: new Date(),
        model: response.model,
        tokensUsed: response.tokens_used,
      };
      setMessages((prev) => [...prev, assistantMessage]);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e);
    }
  };

  return (
    <div className="chat-container">
      <header className="workspace-header">
        <div>
          <h1 className="workspace-title">
            <MessageSquare className="workspace-title-icon" size={20} />
            Chat
          </h1>
          <p className="workspace-subtitle">Converse with Gemini</p>
        </div>
      </header>

      <div className="chat-messages">
        {messages.length === 0 && (
          <div className="empty-state">
            <Sparkles className="empty-state-icon" size={48} />
            <h2 className="empty-state-title">Start a conversation</h2>
            <p className="empty-state-description">
              Ask anything. Genie will use the Gemini CLI to respond.
            </p>
          </div>
        )}

        {messages.map((message) => (
          <div key={message.id} className={`message ${message.role}`}>
            <div className="message-content">{message.content}</div>
            <div className="message-meta">
              {message.model && <span>{message.model}</span>}
              {message.tokensUsed && <span> · ~{message.tokensUsed} tokens</span>}
            </div>
          </div>
        ))}

        {isLoading && (
          <div className="message assistant">
            <div className="message-content">
              <span className="spinner" style={{ display: 'inline-block' }} />
            </div>
          </div>
        )}

        {error && (
          <div className="message assistant">
            <div className="message-content" style={{ color: 'var(--status-error)' }}>
              Error: {error}
            </div>
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      <form className="chat-input-container" onSubmit={handleSubmit}>
        <div className="chat-input-wrapper">
          <textarea
            className="chat-input"
            placeholder="Ask something..."
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            rows={1}
          />
          <button
            type="submit"
            className="btn btn-primary btn-icon"
            disabled={!input.trim() || isLoading}
          >
            <Send size={18} />
          </button>
        </div>
      </form>
    </div>
  );
}

