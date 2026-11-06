import { useState, type FC, type FormEvent } from 'react';
import type { UncommittedFile } from '../types';

interface DirtyTreeDialogProps {
  uncommittedFiles: UncommittedFile[];
  onStash: (message?: string) => Promise<void>;
  onCommit: (message: string) => Promise<void>;
  onCancel: () => void;
  onProceedAnyway: () => void;
  isLoading: boolean;
  error?: string | null;
}

export const DirtyTreeDialog: FC<DirtyTreeDialogProps> = ({
  uncommittedFiles,
  onStash,
  onCommit,
  onCancel,
  onProceedAnyway,
  isLoading,
  error,
}) => {
  const [activeTab, setActiveTab] = useState<'inspect' | 'stash' | 'commit'>('inspect');
  const [stashMessage, setStashMessage] = useState('');
  const [commitMessage, setCommitMessage] = useState('');

  const handleStashSubmit = async (e: FormEvent) => {
    e.preventDefault();
    await onStash(stashMessage.trim() || undefined);
  };

  const handleCommitSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (commitMessage.trim()) {
      await onCommit(commitMessage.trim());
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/85 flex items-center justify-center p-4 font-mono text-xs">
      <div className="bg-[#121214] border border-[#3f3f46] rounded max-w-xl w-full p-5 space-y-4 shadow-2xl">
        {/* Header */}
        <div className="flex items-start justify-between border-b border-[#27272a] pb-3">
          <div className="space-y-0.5">
            <div className="flex items-center gap-2">
              <span className="font-semibold text-zinc-100">[dirty_primary_tree: blocked]</span>
              <span className="text-[10px] text-zinc-400 border border-[#27272a] px-1.5 rounded">
                {uncommittedFiles.length} file(s)
              </span>
            </div>
            <p className="text-[11px] text-zinc-500">
              Primary repository has uncommitted changes. Zencode never silently stashes or overwrites your work.
            </p>
          </div>

          <button
            type="button"
            onClick={onCancel}
            disabled={isLoading}
            className="text-zinc-500 hover:text-zinc-200"
          >
            [x]
          </button>
        </div>

        {error && (
          <div className="p-2.5 bg-[#18181b] border border-rose-900/60 text-rose-300 text-xs">
            [error]: {error}
          </div>
        )}

        {/* Tab Selection */}
        <div className="flex items-center gap-1 border-b border-[#27272a] pb-2 text-xs">
          <button
            type="button"
            onClick={() => setActiveTab('inspect')}
            className={`px-2.5 py-1 rounded transition-colors ${
              activeTab === 'inspect'
                ? 'bg-[#27272a] text-white'
                : 'text-zinc-500 hover:text-zinc-300'
            }`}
          >
            [1: inspect files ({uncommittedFiles.length})]
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('stash')}
            className={`px-2.5 py-1 rounded transition-colors ${
              activeTab === 'stash'
                ? 'bg-[#27272a] text-white'
                : 'text-zinc-500 hover:text-zinc-300'
            }`}
          >
            [2: stash changes]
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('commit')}
            className={`px-2.5 py-1 rounded transition-colors ${
              activeTab === 'commit'
                ? 'bg-[#27272a] text-white'
                : 'text-zinc-500 hover:text-zinc-300'
            }`}
          >
            [3: commit changes]
          </button>
        </div>

        {/* Tab 1: Inspect */}
        {activeTab === 'inspect' && (
          <div className="space-y-3">
            <div className="max-h-52 overflow-y-auto p-2 bg-[#09090b] border border-[#27272a] divide-y divide-[#1f1f23]">
              {uncommittedFiles.map((file, idx) => (
                <div key={idx} className="py-1 flex items-center justify-between text-[11px]">
                  <div className="flex items-center gap-2 truncate">
                    <span className="text-zinc-500 font-bold shrink-0">
                      [{file.status_code}]
                    </span>
                    <span className="text-zinc-200 truncate">{file.path}</span>
                  </div>
                  <span className="text-zinc-600 text-[10px] shrink-0 pl-2">
                    {file.description}
                  </span>
                </div>
              ))}
            </div>

            <div className="text-[11px] text-zinc-500">
              * Select stash or commit above to clean the tree before creating task worktree.
            </div>
          </div>
        )}

        {/* Tab 2: Stash */}
        {activeTab === 'stash' && (
          <form onSubmit={handleStashSubmit} className="space-y-3">
            <div className="space-y-1">
              <label className="text-[11px] text-zinc-400">stash message (optional):</label>
              <input
                type="text"
                value={stashMessage}
                onChange={(e) => setStashMessage(e.target.value)}
                placeholder="work-in-progress before task"
                disabled={isLoading}
                className="w-full px-2.5 py-1.5 bg-[#09090b] border border-[#27272a] text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-zinc-500 rounded text-xs"
              />
            </div>
            <div className="text-[11px] text-zinc-500">
              command: <code className="text-zinc-300">git stash push -u</code>
            </div>
            <button
              type="submit"
              disabled={isLoading}
              className="w-full py-1.5 bg-zinc-200 hover:bg-white text-black font-medium rounded transition-colors disabled:opacity-40"
            >
              {isLoading ? '[stashing...]' : '[stash_and_proceed]'}
            </button>
          </form>
        )}

        {/* Tab 3: Commit */}
        {activeTab === 'commit' && (
          <form onSubmit={handleCommitSubmit} className="space-y-3">
            <div className="space-y-1">
              <label className="text-[11px] text-zinc-400">commit message:</label>
              <input
                type="text"
                value={commitMessage}
                onChange={(e) => setCommitMessage(e.target.value)}
                placeholder="save local changes before starting task"
                disabled={isLoading}
                required
                className="w-full px-2.5 py-1.5 bg-[#09090b] border border-[#27272a] text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-zinc-500 rounded text-xs"
              />
            </div>
            <div className="text-[11px] text-zinc-500">
              command: <code className="text-zinc-300">git add -A && git commit -m &quot;...&quot;</code>
            </div>
            <button
              type="submit"
              disabled={isLoading || !commitMessage.trim()}
              className="w-full py-1.5 bg-zinc-200 hover:bg-white text-black font-medium rounded transition-colors disabled:opacity-40"
            >
              {isLoading ? '[committing...]' : '[commit_and_proceed]'}
            </button>
          </form>
        )}

        {/* Footer actions */}
        <div className="pt-3 border-t border-[#27272a] flex items-center justify-between text-xs">
          <button
            type="button"
            onClick={onCancel}
            disabled={isLoading}
            className="px-2.5 py-1 rounded bg-[#18181b] hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200"
          >
            [cancel_task]
          </button>

          <button
            type="button"
            onClick={onProceedAnyway}
            disabled={isLoading}
            title="Explicit acknowledgment: uncommitted changes stay in primary tree and will not be in task worktree"
            className="px-2.5 py-1 rounded bg-[#18181b] hover:bg-[#27272a] text-zinc-300 hover:text-white border border-[#27272a]"
          >
            [proceed_anyway_with_acknowledgment &gt;]
          </button>
        </div>
      </div>
    </div>
  );
};
