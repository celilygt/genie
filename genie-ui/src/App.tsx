import { useState } from 'react';
import './styles/globals.css';
import { Sidebar } from './components/Sidebar';
import { ChatWorkspace } from './components/ChatWorkspace';
import { DocsWorkspace } from './components/DocsWorkspace';
import { RepoWorkspace } from './components/RepoWorkspace';
import { PromptsWorkspace } from './components/PromptsWorkspace';
import { QuotaWorkspace } from './components/QuotaWorkspace';
import type { Workspace } from './types';

function App() {
  const [workspace, setWorkspace] = useState<Workspace>('chat');

  const renderWorkspace = () => {
    switch (workspace) {
      case 'chat':
        return <ChatWorkspace />;
      case 'docs':
        return <DocsWorkspace />;
      case 'repo':
        return <RepoWorkspace />;
      case 'prompts':
        return <PromptsWorkspace />;
      case 'quota':
        return <QuotaWorkspace />;
      default:
        return <ChatWorkspace />;
    }
  };

  return (
    <div className="app-container">
      <Sidebar activeWorkspace={workspace} onWorkspaceChange={setWorkspace} />
      <main className="main-content">{renderWorkspace()}</main>
    </div>
  );
}

export default App;
