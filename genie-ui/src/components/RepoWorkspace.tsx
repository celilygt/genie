import { useState } from 'react';
import { FolderGit2, FolderOpen, CheckCircle2, Code2, FileCode } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { useRepo } from '../hooks/useGenie';
import type { RepoSummary } from '../types';

export function RepoWorkspace() {
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [maxFiles, setMaxFiles] = useState(100);
  const [summary, setSummary] = useState<RepoSummary | null>(null);
  const { summarizeRepo, isLoading, error } = useRepo();

  const handleSelectFolder = async () => {
    const folder = await open({
      multiple: false,
      directory: true,
    });
    if (folder) {
      setSelectedPath(folder);
      setSummary(null);
    }
  };

  const handleSummarize = async () => {
    if (!selectedPath) return;
    const result = await summarizeRepo(selectedPath, maxFiles);
    if (result) setSummary(result);
  };

  return (
    <div className="chat-container">
      <header className="workspace-header">
        <div>
          <h1 className="workspace-title">
            <FolderGit2 className="workspace-title-icon" size={20} />
            Repository
          </h1>
          <p className="workspace-subtitle">Analyze and summarize codebases</p>
        </div>
      </header>

      <div className="workspace-content">
        {/* Folder Selection */}
        <div className="workspace-section">
          <div className="section-title">Select Repository</div>
          <div className="drop-zone" onClick={handleSelectFolder}>
            <FolderOpen className="drop-zone-icon" size={32} />
            <p className="drop-zone-text">
              {selectedPath || 'Click to select a repository folder'}
            </p>
            <p className="drop-zone-hint">
              Respects .gitignore · Supports 30+ languages
            </p>
          </div>
        </div>

        {/* Options */}
        <div className="workspace-section">
          <div className="section-title">Options</div>
          <div>
            <label className="label">Max Files to Analyze</label>
            <input
              type="number"
              className="input"
              value={maxFiles}
              onChange={(e) => setMaxFiles(Number(e.target.value))}
              min={10}
              max={500}
              style={{ width: 120 }}
            />
          </div>
        </div>

        {/* Action */}
        <div className="workspace-section">
          <button
            className="btn btn-primary"
            onClick={handleSummarize}
            disabled={!selectedPath || isLoading}
          >
            {isLoading ? (
              <>
                <span className="spinner" />
                Analyzing...
              </>
            ) : (
              <>
                <CheckCircle2 size={16} />
                Analyze Repository
              </>
            )}
          </button>
        </div>

        {error && (
          <div className="card" style={{ borderColor: 'var(--status-error)' }}>
            <p style={{ color: 'var(--status-error)' }}>{error}</p>
          </div>
        )}

        {/* Summary Result */}
        {summary && (
          <div className="workspace-section">
            <div className="section-title">Analysis Result</div>
            
            {/* Overview Card */}
            <div className="card" style={{ marginBottom: 'var(--space-md)' }}>
              <div className="card-header">
                <div className="card-icon">
                  <Code2 size={20} />
                </div>
                <div>
                  <h3 className="card-title">{summary.name}</h3>
                  <p className="card-description">
                    {summary.file_count} files · {summary.total_lines.toLocaleString()} lines
                  </p>
                </div>
              </div>
              <p style={{ lineHeight: 1.7 }}>{summary.overview}</p>
              
              {/* Languages */}
              <div style={{ display: 'flex', gap: 'var(--space-xs)', flexWrap: 'wrap', marginTop: 'var(--space-md)' }}>
                {summary.languages.map((lang) => (
                  <span key={lang} className="badge badge-success">{lang}</span>
                ))}
              </div>
            </div>

            {/* Modules */}
            {summary.modules.map((module) => (
              <div key={module.path} className="list-item" style={{ marginBottom: 'var(--space-sm)' }}>
                <FileCode className="list-item-icon" size={20} />
                <div className="list-item-content">
                  <div className="list-item-title">{module.path}</div>
                  <div className="list-item-meta">{module.description}</div>
                  {module.key_files.length > 0 && (
                    <div style={{ marginTop: 'var(--space-xs)', display: 'flex', gap: 'var(--space-xs)', flexWrap: 'wrap' }}>
                      {module.key_files.map((file) => (
                        <span key={file} style={{ fontSize: 11, color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)' }}>
                          {file}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

