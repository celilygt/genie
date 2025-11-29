import { useEffect } from 'react';
import { Gauge, RefreshCw, Clock, Zap, MessageSquare, CheckCircle2, XCircle } from 'lucide-react';
import { useQuota, useHealth } from '../hooks/useGenie';

export function QuotaWorkspace() {
  const { status, usageLog, fetchStatus, fetchUsageLog, isLoading } = useQuota();
  const { health, checkHealth } = useHealth();

  useEffect(() => {
    fetchStatus();
    fetchUsageLog(20);
    checkHealth();
  }, [fetchStatus, fetchUsageLog, checkHealth]);

  const handleRefresh = () => {
    fetchStatus();
    fetchUsageLog(20);
  };

  const dayPercentage = status
    ? (status.requests_today / status.requests_per_day_limit) * 100
    : 0;
  
  const minutePercentage = status
    ? (status.requests_last_minute / status.requests_per_minute_limit) * 100
    : 0;

  return (
    <div className="chat-container">
      <header className="workspace-header">
        <div>
          <h1 className="workspace-title">
            <Gauge className="workspace-title-icon" size={20} />
            Quota & Usage
          </h1>
          <p className="workspace-subtitle">Monitor your Gemini API usage</p>
        </div>
        <button className="btn btn-ghost" onClick={handleRefresh} disabled={isLoading}>
          <RefreshCw size={16} className={isLoading ? 'spinner' : ''} />
        </button>
      </header>

      <div className="workspace-content">
        {/* Health Status */}
        <div className="workspace-section">
          <div className="section-title">System Status</div>
          <div className="card">
            <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--space-md)' }}>
              <div
                style={{
                  width: 12,
                  height: 12,
                  borderRadius: '50%',
                  background:
                    health?.status === 'ok'
                      ? 'var(--status-success)'
                      : 'var(--status-warning)',
                  boxShadow:
                    health?.status === 'ok'
                      ? '0 0 8px var(--status-success)'
                      : '0 0 8px var(--status-warning)',
                }}
              />
              <span>
                {health?.status === 'ok' ? 'All systems operational' : 'Degraded performance'}
              </span>
              {health && (
                <span className="badge badge-success">v{health.version}</span>
              )}
            </div>
            <div style={{ marginTop: 'var(--space-md)', display: 'flex', gap: 'var(--space-lg)' }}>
              <div>
                <span style={{ color: 'var(--text-tertiary)', fontSize: 12 }}>Gemini CLI</span>
                <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--space-xs)' }}>
                  {health?.gemini_available ? (
                    <CheckCircle2 size={14} style={{ color: 'var(--status-success)' }} />
                  ) : (
                    <XCircle size={14} style={{ color: 'var(--status-error)' }} />
                  )}
                  <span>{health?.gemini_available ? 'Available' : 'Unavailable'}</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Stats */}
        {status && (
          <div className="stats-grid" style={{ padding: 0, marginBottom: 'var(--space-xl)' }}>
            <div className="stat-card">
              <div className="stat-label">Requests Today</div>
              <div className="stat-value">
                {status.requests_today}
                <span className="stat-unit">/ {status.requests_per_day_limit}</span>
              </div>
              <div className="progress-bar" style={{ marginTop: 'var(--space-sm)' }}>
                <div
                  className="progress-bar-fill"
                  style={{
                    width: `${Math.min(dayPercentage, 100)}%`,
                    background: dayPercentage > 80 ? 'var(--status-warning)' : undefined,
                  }}
                />
              </div>
            </div>

            <div className="stat-card">
              <div className="stat-label">Requests (Last Minute)</div>
              <div className="stat-value">
                {status.requests_last_minute}
                <span className="stat-unit">/ {status.requests_per_minute_limit}</span>
              </div>
              <div className="progress-bar" style={{ marginTop: 'var(--space-sm)' }}>
                <div
                  className="progress-bar-fill"
                  style={{
                    width: `${Math.min(minutePercentage, 100)}%`,
                    background: minutePercentage > 80 ? 'var(--status-warning)' : undefined,
                  }}
                />
              </div>
            </div>

            <div className="stat-card">
              <div className="stat-label">Input Tokens Today</div>
              <div className="stat-value accent">
                {status.approx_input_tokens_today.toLocaleString()}
              </div>
            </div>

            <div className="stat-card">
              <div className="stat-label">Output Tokens Today</div>
              <div className="stat-value accent">
                {status.approx_output_tokens_today.toLocaleString()}
              </div>
            </div>
          </div>
        )}

        {/* Reset Time */}
        {status && (
          <div className="workspace-section">
            <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--space-sm)', color: 'var(--text-tertiary)' }}>
              <Clock size={14} />
              <span>Quota resets at {status.reset_time}</span>
            </div>
          </div>
        )}

        {/* Usage Log */}
        <div className="workspace-section">
          <div className="section-title">Recent Activity</div>
          {usageLog.length === 0 ? (
            <div className="empty-state" style={{ padding: 'var(--space-xl)' }}>
              <Zap className="empty-state-icon" size={32} />
              <p className="empty-state-description">No recent activity</p>
            </div>
          ) : (
            <div className="list">
              {usageLog.map((entry) => (
                <div key={entry.id} className="list-item">
                  <MessageSquare
                    className="list-item-icon"
                    size={16}
                    style={{
                      color: entry.success ? 'var(--accent-primary)' : 'var(--status-error)',
                    }}
                  />
                  <div className="list-item-content">
                    <div className="list-item-title">
                      {entry.kind}
                      <span
                        className={`badge ${entry.success ? 'badge-success' : 'badge-error'}`}
                        style={{ marginLeft: 'var(--space-sm)' }}
                      >
                        {entry.success ? 'success' : 'failed'}
                      </span>
                    </div>
                    <div className="list-item-meta">
                      {entry.model} · {entry.approx_input_tokens + entry.approx_output_tokens} tokens ·{' '}
                      {new Date(entry.timestamp).toLocaleTimeString()}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

