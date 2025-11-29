import { useEffect, useState } from 'react';
import { LayoutTemplate, Play, RefreshCw, Variable } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { useTemplates } from '../hooks/useGenie';
import type { TemplateInfo, VariableInfo } from '../types';

export function PromptsWorkspace() {
  const { templates, fetchTemplates, runTemplate, isLoading, error } = useTemplates();
  const [selectedTemplate, setSelectedTemplate] = useState<TemplateInfo | null>(null);
  const [variables, setVariables] = useState<Record<string, string>>({});
  const [files, setFiles] = useState<Record<string, string>>({});
  const [output, setOutput] = useState<string | null>(null);

  useEffect(() => {
    fetchTemplates();
  }, [fetchTemplates]);

  const handleSelectTemplate = (template: TemplateInfo) => {
    setSelectedTemplate(template);
    setOutput(null);
    // Initialize variables with defaults
    const initialVars: Record<string, string> = {};
    template.variables.forEach((v) => {
      if (v.default) initialVars[v.name] = v.default;
    });
    setVariables(initialVars);
    setFiles({});
  };

  const handleSelectFile = async (varName: string) => {
    const file = await open({ multiple: false });
    if (file) {
      setFiles((prev) => ({ ...prev, [varName]: file }));
    }
  };

  const handleRun = async () => {
    if (!selectedTemplate) return;
    const result = await runTemplate(selectedTemplate.name, variables, files);
    if (result) setOutput(result);
  };

  const renderVariableInput = (variable: VariableInfo) => {
    if (variable.var_type === 'file') {
      return (
        <div key={variable.name} style={{ marginBottom: 'var(--space-md)' }}>
          <label className="label">
            {variable.name}
            {variable.required && <span style={{ color: 'var(--status-error)' }}> *</span>}
          </label>
          <button
            className="btn btn-secondary"
            onClick={() => handleSelectFile(variable.name)}
            style={{ width: '100%', justifyContent: 'flex-start' }}
          >
            {files[variable.name] || 'Select file...'}
          </button>
          <p style={{ fontSize: 12, color: 'var(--text-tertiary)', marginTop: 'var(--space-xs)' }}>
            {variable.description}
          </p>
        </div>
      );
    }

    if (variable.var_type === 'boolean') {
      return (
        <div key={variable.name} style={{ marginBottom: 'var(--space-md)' }}>
          <label style={{ display: 'flex', alignItems: 'center', gap: 'var(--space-sm)' }}>
            <input
              type="checkbox"
              checked={variables[variable.name] === 'true'}
              onChange={(e) =>
                setVariables((prev) => ({
                  ...prev,
                  [variable.name]: e.target.checked ? 'true' : 'false',
                }))
              }
            />
            <span className="label" style={{ margin: 0 }}>{variable.name}</span>
          </label>
          <p style={{ fontSize: 12, color: 'var(--text-tertiary)', marginTop: 'var(--space-xs)' }}>
            {variable.description}
          </p>
        </div>
      );
    }

    if (variable.var_type.startsWith('enum:')) {
      const options = variable.var_type.replace('enum:', '').split(',');
      return (
        <div key={variable.name} style={{ marginBottom: 'var(--space-md)' }}>
          <label className="label">
            {variable.name}
            {variable.required && <span style={{ color: 'var(--status-error)' }}> *</span>}
          </label>
          <select
            className="input select"
            value={variables[variable.name] || ''}
            onChange={(e) =>
              setVariables((prev) => ({ ...prev, [variable.name]: e.target.value }))
            }
          >
            <option value="">Select...</option>
            {options.map((opt) => (
              <option key={opt} value={opt}>
                {opt}
              </option>
            ))}
          </select>
          <p style={{ fontSize: 12, color: 'var(--text-tertiary)', marginTop: 'var(--space-xs)' }}>
            {variable.description}
          </p>
        </div>
      );
    }

    return (
      <div key={variable.name} style={{ marginBottom: 'var(--space-md)' }}>
        <label className="label">
          {variable.name}
          {variable.required && <span style={{ color: 'var(--status-error)' }}> *</span>}
        </label>
        <input
          type={variable.var_type === 'number' ? 'number' : 'text'}
          className="input"
          placeholder={variable.default || variable.description}
          value={variables[variable.name] || ''}
          onChange={(e) =>
            setVariables((prev) => ({ ...prev, [variable.name]: e.target.value }))
          }
        />
        <p style={{ fontSize: 12, color: 'var(--text-tertiary)', marginTop: 'var(--space-xs)' }}>
          {variable.description}
        </p>
      </div>
    );
  };

  return (
    <div className="chat-container">
      <header className="workspace-header">
        <div>
          <h1 className="workspace-title">
            <LayoutTemplate className="workspace-title-icon" size={20} />
            Prompts
          </h1>
          <p className="workspace-subtitle">Reusable prompt templates</p>
        </div>
        <button className="btn btn-ghost" onClick={fetchTemplates}>
          <RefreshCw size={16} />
        </button>
      </header>

      <div className="workspace-content" style={{ display: 'flex', gap: 'var(--space-xl)' }}>
        {/* Template List */}
        <div style={{ width: 280, flexShrink: 0 }}>
          <div className="section-title">Templates</div>
          <div className="list">
            {templates.length === 0 && !isLoading && (
              <p style={{ color: 'var(--text-tertiary)', fontSize: 13 }}>
                No templates found. Add .prompt.md files to ~/.genie/prompts/
              </p>
            )}
            {templates.map((template) => (
              <div
                key={template.name}
                className={`list-item ${selectedTemplate?.name === template.name ? 'active' : ''}`}
                onClick={() => handleSelectTemplate(template)}
                style={{
                  borderColor:
                    selectedTemplate?.name === template.name
                      ? 'var(--accent-primary)'
                      : undefined,
                }}
              >
                <LayoutTemplate className="list-item-icon" size={18} />
                <div className="list-item-content">
                  <div className="list-item-title">{template.name}</div>
                  <div className="list-item-meta">{template.description}</div>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Template Details */}
        <div style={{ flex: 1 }}>
          {!selectedTemplate ? (
            <div className="empty-state">
              <Variable className="empty-state-icon" size={48} />
              <h2 className="empty-state-title">Select a template</h2>
              <p className="empty-state-description">
                Choose a template from the list to configure and run it.
              </p>
            </div>
          ) : (
            <>
              <div className="section-title">Configure: {selectedTemplate.name}</div>
              <div className="card" style={{ marginBottom: 'var(--space-lg)' }}>
                <p style={{ marginBottom: 'var(--space-md)', color: 'var(--text-secondary)' }}>
                  {selectedTemplate.description}
                </p>
                {selectedTemplate.model && (
                  <span className="badge badge-success" style={{ marginBottom: 'var(--space-md)' }}>
                    {selectedTemplate.model}
                  </span>
                )}

                {selectedTemplate.variables.length > 0 && (
                  <div style={{ marginTop: 'var(--space-lg)' }}>
                    <h4 style={{ marginBottom: 'var(--space-md)' }}>Variables</h4>
                    {selectedTemplate.variables.map(renderVariableInput)}
                  </div>
                )}

                <button
                  className="btn btn-primary"
                  onClick={handleRun}
                  disabled={isLoading}
                  style={{ marginTop: 'var(--space-md)' }}
                >
                  {isLoading ? (
                    <>
                      <span className="spinner" />
                      Running...
                    </>
                  ) : (
                    <>
                      <Play size={16} />
                      Run Template
                    </>
                  )}
                </button>
              </div>

              {error && (
                <div className="card" style={{ borderColor: 'var(--status-error)', marginBottom: 'var(--space-lg)' }}>
                  <p style={{ color: 'var(--status-error)' }}>{error}</p>
                </div>
              )}

              {output && (
                <div className="workspace-section">
                  <div className="section-title">Output</div>
                  <div className="card">
                    <pre
                      className="code-block"
                      style={{ whiteSpace: 'pre-wrap', maxHeight: 400, overflow: 'auto' }}
                    >
                      {output}
                    </pre>
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

