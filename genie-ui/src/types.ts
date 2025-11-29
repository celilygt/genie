// Type definitions for Genie UI

export type Workspace = 'chat' | 'docs' | 'repo' | 'prompts' | 'quota';

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  model?: string;
  tokensUsed?: number;
}

export interface ChatRequest {
  message: string;
  systemPrompt?: string;
  model?: string;
}

export interface ChatResponse {
  message: string;
  model: string;
  tokens_used: number;
}

export interface QuotaStatus {
  requests_today: number;
  requests_per_day_limit: number;
  requests_last_minute: number;
  requests_per_minute_limit: number;
  approx_input_tokens_today: number;
  approx_output_tokens_today: number;
  last_error?: string;
  reset_time: string;
}

export interface UsageLogEntry {
  id: number;
  timestamp: string;
  model: string;
  kind: string;
  prompt_chars: number;
  response_chars: number;
  approx_input_tokens: number;
  approx_output_tokens: number;
  success: boolean;
  error_code?: string;
}

export interface DocumentSummary {
  title?: string;
  summary: string;
  key_points: string[];
  word_count?: number;
  page_count?: number;
}

export interface ChapterSummary {
  chapter_id: number;
  title: string;
  summary: string;
  key_points: string[];
  important_terms: string[];
  questions_for_reflection: string[];
}

export interface BookSummary {
  title?: string;
  author?: string;
  chapters: ChapterSummary[];
  global_summary: string;
  reading_roadmap: string[];
  page_count?: number;
}

export interface RepoSummary {
  name: string;
  overview: string;
  modules: ModuleSummary[];
  languages: string[];
  file_count: number;
  total_lines: number;
}

export interface ModuleSummary {
  path: string;
  description: string;
  key_files: string[];
  technologies: string[];
}

export interface TemplateInfo {
  name: string;
  description: string;
  model?: string;
  json_output: boolean;
  variables: VariableInfo[];
}

export interface VariableInfo {
  name: string;
  description: string;
  var_type: string;
  default?: string;
  required: boolean;
}

export interface HealthStatus {
  status: string;
  version: string;
  gemini_available: boolean;
}

export interface AppConfig {
  gemini_binary: string;
  default_model: string;
  server_host: string;
  server_port: number;
  quota_per_minute: number;
  quota_per_day: number;
  quota_reset_time: string;
}

