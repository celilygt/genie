import {
  MessageSquare,
  FileText,
  FolderGit2,
  LayoutTemplate,
  Gauge,
  Settings,
} from 'lucide-react';
import type { Workspace } from '../types';

interface SidebarProps {
  activeWorkspace: Workspace;
  onWorkspaceChange: (workspace: Workspace) => void;
}

const navItems: { id: Workspace; icon: typeof MessageSquare; label: string }[] = [
  { id: 'chat', icon: MessageSquare, label: 'Chat' },
  { id: 'docs', icon: FileText, label: 'Documents' },
  { id: 'repo', icon: FolderGit2, label: 'Repository' },
  { id: 'prompts', icon: LayoutTemplate, label: 'Prompts' },
  { id: 'quota', icon: Gauge, label: 'Quota' },
];

export function Sidebar({ activeWorkspace, onWorkspaceChange }: SidebarProps) {
  return (
    <nav className="sidebar">
      <div className="sidebar-logo" />
      
      {navItems.map((item) => (
        <button
          key={item.id}
          className={`nav-item ${activeWorkspace === item.id ? 'active' : ''}`}
          onClick={() => onWorkspaceChange(item.id)}
          data-tooltip={item.label}
        >
          <item.icon size={20} />
        </button>
      ))}

      <div className="sidebar-bottom">
        <button className="nav-item" data-tooltip="Settings">
          <Settings size={20} />
        </button>
      </div>
    </nav>
  );
}

