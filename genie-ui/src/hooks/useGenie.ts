import { invoke } from '@tauri-apps/api/core';
import { useState, useCallback } from 'react';
import type {
  ChatRequest,
  ChatResponse,
  QuotaStatus,
  UsageLogEntry,
  DocumentSummary,
  BookSummary,
  RepoSummary,
  TemplateInfo,
  HealthStatus,
  AppConfig,
} from '../types';

// Chat hook
export function useChat() {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const sendMessage = useCallback(async (request: ChatRequest): Promise<ChatResponse | null> => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await invoke<ChatResponse>('send_message', { request });
      return response;
    } catch (e) {
      const err = e as { message?: string };
      setError(err.message || 'Failed to send message');
      return null;
    } finally {
      setIsLoading(false);
    }
  }, []);

  return { sendMessage, isLoading, error };
}

// Quota hook
export function useQuota() {
  const [status, setStatus] = useState<QuotaStatus | null>(null);
  const [usageLog, setUsageLog] = useState<UsageLogEntry[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchStatus = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await invoke<QuotaStatus>('get_quota_status');
      setStatus(result);
    } catch (e) {
      const err = e as { message?: string };
      setError(err.message || 'Failed to fetch quota status');
    } finally {
      setIsLoading(false);
    }
  }, []);

  const fetchUsageLog = useCallback(async (limit?: number) => {
    try {
      const result = await invoke<UsageLogEntry[]>('get_usage_log', { limit });
      setUsageLog(result);
    } catch (e) {
      const err = e as { message?: string };
      setError(err.message || 'Failed to fetch usage log');
    }
  }, []);

  return { status, usageLog, fetchStatus, fetchUsageLog, isLoading, error };
}

// Docs hook
export function useDocs() {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const summarizePdf = useCallback(async (
    path: string,
    style?: string,
    language?: string
  ): Promise<DocumentSummary | null> => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await invoke<DocumentSummary>('summarize_pdf', {
        request: { path, style, language },
      });
      return result;
    } catch (e) {
      const err = e as { message?: string };
      setError(err.message || 'Failed to summarize PDF');
      return null;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const summarizeBook = useCallback(async (
    path: string,
    style?: string,
    language?: string,
    useGeminiChapters?: boolean
  ): Promise<BookSummary | null> => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await invoke<BookSummary>('summarize_book', {
        request: { path, style, language, use_gemini_chapters: useGeminiChapters },
      });
      return result;
    } catch (e) {
      const err = e as { message?: string };
      setError(err.message || 'Failed to summarize book');
      return null;
    } finally {
      setIsLoading(false);
    }
  }, []);

  return { summarizePdf, summarizeBook, isLoading, error };
}

// Repo hook
export function useRepo() {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const summarizeRepo = useCallback(async (
    path: string,
    maxFiles?: number
  ): Promise<RepoSummary | null> => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await invoke<RepoSummary>('summarize_repo', {
        request: { path, max_files: maxFiles },
      });
      return result;
    } catch (e) {
      const err = e as { message?: string };
      setError(err.message || 'Failed to summarize repository');
      return null;
    } finally {
      setIsLoading(false);
    }
  }, []);

  return { summarizeRepo, isLoading, error };
}

// Templates hook
export function useTemplates() {
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchTemplates = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await invoke<TemplateInfo[]>('list_templates');
      setTemplates(result);
    } catch (e) {
      const err = e as { message?: string };
      setError(err.message || 'Failed to fetch templates');
    } finally {
      setIsLoading(false);
    }
  }, []);

  const runTemplate = useCallback(async (
    name: string,
    variables: Record<string, string>,
    files: Record<string, string>
  ): Promise<string | null> => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await invoke<{ output: string }>('run_template', {
        request: { name, variables, files },
      });
      return result.output;
    } catch (e) {
      const err = e as { message?: string };
      setError(err.message || 'Failed to run template');
      return null;
    } finally {
      setIsLoading(false);
    }
  }, []);

  return { templates, fetchTemplates, runTemplate, isLoading, error };
}

// Config & Health hook
export function useHealth() {
  const [health, setHealth] = useState<HealthStatus | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const checkHealth = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await invoke<HealthStatus>('health_check');
      setHealth(result);
    } catch {
      setHealth({ status: 'error', version: 'unknown', gemini_available: false });
    } finally {
      setIsLoading(false);
    }
  }, []);

  const fetchConfig = useCallback(async () => {
    try {
      const result = await invoke<AppConfig>('get_config');
      setConfig(result);
    } catch {
      // Ignore
    }
  }, []);

  return { health, config, checkHealth, fetchConfig, isLoading };
}

