import { useState } from 'react';
import { FileText, Upload, BookOpen, CheckCircle2 } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { useDocs } from '../hooks/useGenie';
import type { DocumentSummary, BookSummary } from '../types';

type Mode = 'pdf' | 'book';

export function DocsWorkspace() {
  const [mode, setMode] = useState<Mode>('pdf');
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [style, setStyle] = useState('concise');
  const [pdfSummary, setPdfSummary] = useState<DocumentSummary | null>(null);
  const [bookSummary, setBookSummary] = useState<BookSummary | null>(null);
  const { summarizePdf, summarizeBook, isLoading, error } = useDocs();

  const handleSelectFile = async () => {
    const file = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (file) {
      setSelectedFile(file);
      setPdfSummary(null);
      setBookSummary(null);
    }
  };

  const handleSummarize = async () => {
    if (!selectedFile) return;

    if (mode === 'pdf') {
      const result = await summarizePdf(selectedFile, style);
      if (result) setPdfSummary(result);
    } else {
      const result = await summarizeBook(selectedFile, style);
      if (result) setBookSummary(result);
    }
  };

  return (
    <div className="chat-container">
      <header className="workspace-header">
        <div>
          <h1 className="workspace-title">
            <FileText className="workspace-title-icon" size={20} />
            Documents
          </h1>
          <p className="workspace-subtitle">Summarize PDFs and books</p>
        </div>
      </header>

      <div className="workspace-content">
        {/* Mode Selection */}
        <div className="workspace-section">
          <div className="section-title">Mode</div>
          <div style={{ display: 'flex', gap: 'var(--space-sm)' }}>
            <button
              className={`btn ${mode === 'pdf' ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setMode('pdf')}
            >
              <FileText size={16} />
              PDF Summary
            </button>
            <button
              className={`btn ${mode === 'book' ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setMode('book')}
            >
              <BookOpen size={16} />
              Book Summary
            </button>
          </div>
        </div>

        {/* File Selection */}
        <div className="workspace-section">
          <div className="section-title">Select File</div>
          <div
            className={`drop-zone ${selectedFile ? '' : ''}`}
            onClick={handleSelectFile}
          >
            <Upload className="drop-zone-icon" size={32} />
            <p className="drop-zone-text">
              {selectedFile || 'Click to select a PDF file'}
            </p>
            <p className="drop-zone-hint">
              {mode === 'pdf' ? 'Single document summarization' : 'Chapter-based book summarization'}
            </p>
          </div>
        </div>

        {/* Options */}
        <div className="workspace-section">
          <div className="section-title">Options</div>
          <div style={{ display: 'flex', gap: 'var(--space-md)', flexWrap: 'wrap' }}>
            <div>
              <label className="label">Summary Style</label>
              <select
                className="input select"
                value={style}
                onChange={(e) => setStyle(e.target.value)}
                style={{ width: 180 }}
              >
                <option value="concise">Concise</option>
                <option value="detailed">Detailed</option>
                <option value="exam-notes">Exam Notes</option>
                <option value="bullet">Bullet Points</option>
              </select>
            </div>
          </div>
        </div>

        {/* Action */}
        <div className="workspace-section">
          <button
            className="btn btn-primary"
            onClick={handleSummarize}
            disabled={!selectedFile || isLoading}
          >
            {isLoading ? (
              <>
                <span className="spinner" />
                Processing...
              </>
            ) : (
              <>
                <CheckCircle2 size={16} />
                Summarize
              </>
            )}
          </button>
        </div>

        {error && (
          <div className="card" style={{ borderColor: 'var(--status-error)' }}>
            <p style={{ color: 'var(--status-error)' }}>{error}</p>
          </div>
        )}

        {/* PDF Summary Result */}
        {pdfSummary && (
          <div className="workspace-section">
            <div className="section-title">Summary</div>
            <div className="card">
              {pdfSummary.title && (
                <h3 style={{ marginBottom: 'var(--space-md)' }}>{pdfSummary.title}</h3>
              )}
              <p style={{ marginBottom: 'var(--space-md)', lineHeight: 1.7 }}>
                {pdfSummary.summary}
              </p>
              {pdfSummary.key_points.length > 0 && (
                <>
                  <h4 style={{ marginBottom: 'var(--space-sm)', color: 'var(--accent-primary)' }}>
                    Key Points
                  </h4>
                  <ul style={{ paddingLeft: 'var(--space-lg)' }}>
                    {pdfSummary.key_points.map((point, i) => (
                      <li key={i} style={{ marginBottom: 'var(--space-xs)' }}>{point}</li>
                    ))}
                  </ul>
                </>
              )}
              {pdfSummary.page_count && (
                <p className="message-meta" style={{ marginTop: 'var(--space-md)' }}>
                  {pdfSummary.page_count} pages
                </p>
              )}
            </div>
          </div>
        )}

        {/* Book Summary Result */}
        {bookSummary && (
          <div className="workspace-section">
            <div className="section-title">Book Summary</div>
            
            {/* Global Summary */}
            <div className="card" style={{ marginBottom: 'var(--space-md)' }}>
              {bookSummary.title && (
                <h3 style={{ marginBottom: 'var(--space-xs)' }}>{bookSummary.title}</h3>
              )}
              {bookSummary.author && (
                <p style={{ color: 'var(--text-secondary)', marginBottom: 'var(--space-md)' }}>
                  by {bookSummary.author}
                </p>
              )}
              <p style={{ lineHeight: 1.7 }}>{bookSummary.global_summary}</p>
            </div>

            {/* Chapters */}
            {bookSummary.chapters.map((chapter) => (
              <div key={chapter.chapter_id} className="card" style={{ marginBottom: 'var(--space-md)' }}>
                <h4 style={{ marginBottom: 'var(--space-sm)', color: 'var(--accent-primary)' }}>
                  {chapter.title}
                </h4>
                <p style={{ marginBottom: 'var(--space-md)', lineHeight: 1.7 }}>
                  {chapter.summary}
                </p>
                {chapter.key_points.length > 0 && (
                  <ul style={{ paddingLeft: 'var(--space-lg)', marginBottom: 'var(--space-sm)' }}>
                    {chapter.key_points.map((point, i) => (
                      <li key={i} style={{ marginBottom: 'var(--space-xs)' }}>{point}</li>
                    ))}
                  </ul>
                )}
                {chapter.important_terms.length > 0 && (
                  <div style={{ display: 'flex', gap: 'var(--space-xs)', flexWrap: 'wrap' }}>
                    {chapter.important_terms.map((term, i) => (
                      <span key={i} className="badge badge-success">{term}</span>
                    ))}
                  </div>
                )}
              </div>
            ))}

            {/* Reading Roadmap */}
            {bookSummary.reading_roadmap.length > 0 && (
              <div className="card">
                <h4 style={{ marginBottom: 'var(--space-sm)', color: 'var(--accent-secondary)' }}>
                  📚 Reading Roadmap
                </h4>
                <ol style={{ paddingLeft: 'var(--space-lg)' }}>
                  {bookSummary.reading_roadmap.map((step, i) => (
                    <li key={i} style={{ marginBottom: 'var(--space-xs)' }}>{step}</li>
                  ))}
                </ol>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

