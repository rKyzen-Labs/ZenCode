import { useState, type FC, type FormEvent } from 'react';
import type { UncommittedFile } from '../types';
import {
  AlertTriangle,
  Archive,
  GitCommit,
  X,
  ShieldAlert,
  ArrowRight,
} from 'lucide-react';

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
  const [activeTab, setActiveTab] = useState<'stash' | 'commit' | 'inspect'>('inspect');
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
    <div className="fixed inset-0 z-50 bg-black/75 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-zinc-900 border border-amber-500/40 rounded-xl max-w-xl w-full p-6 shadow-2xl space-y-5">
        {/* Header */}
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-full bg-amber-500/10 border border-amber-500/30 flex items-center justify-center text-amber-400 shrink-0">
              <ShieldAlert className="w-5 h-5" />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-zinc-100">
                Primary Working Tree Has Uncommitted Changes
              </h3>
              <p className="text-xs text-zinc-400">
                Task blocked: {uncommittedFiles.length} uncommitted file(s) detected
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={onCancel}
            disabled={isLoading}
            className="text-zinc-500 hover:text-zinc-300 transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {error && (
          <div className="p-3 rounded-lg bg-rose-500/10 border border-rose-500/30 text-rose-400 text-xs flex items-center gap-2">
            <AlertTriangle className="w-4 h-4 shrink-0" />
            <span>{error}</span>
          </div>
        )}

        <p className="text-xs text-zinc-300 leading-relaxed">
          Zencode never silently modifies, stashes, or overwrites your uncommitted work.
          Because isolated worktrees branch from HEAD, choose how to handle your current changes before proceeding:
        </p>

        {/* Tab Navigation */}
        <div className="flex items-center gap-2 border-b border-zinc-800 pb-2 text-xs">
          <button
            type="button"
            onClick={() => setActiveTab('inspect')}
            className={`px-3 py-1.5 rounded-lg font-medium transition-colors ${
              activeTab === 'inspect'
                ? 'bg-zinc-800 text-zinc-100 border border-zinc-700'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            Uncommitted Files ({uncommittedFiles.length})
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('stash')}
            className={`px-3 py-1.5 rounded-lg font-medium transition-colors flex items-center gap-1.5 ${
              activeTab === 'stash'
                ? 'bg-blue-600/20 text-blue-300 border border-blue-500/40'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            <Archive className="w-3.5 h-3.5" />
            <span>Option 1: Stash</span>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('commit')}
            className={`px-3 py-1.5 rounded-lg font-medium transition-colors flex items-center gap-1.5 ${
              activeTab === 'commit'
                ? 'bg-emerald-600/20 text-emerald-300 border border-emerald-500/40'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            <GitCommit className="w-3.5 h-3.5" />
            <span>Option 2: Commit</span>
          </button>
        </div>

        {/* Tab Content */}
        {activeTab === 'inspect' && (
          <div className="space-y-3">
            <div className="max-h-56 overflow-y-auto p-3 rounded-lg bg-zinc-950 border border-zinc-800 divide-y divide-zinc-900 font-mono text-xs">
              {uncommittedFiles.map((file, idx) => (
                <div key={idx} className="py-1.5 first:pt-0 last:pb-0 flex items-center justify-between">
                  <div className="flex items-center gap-2 truncate">
                    <span className="text-[10px] px-1.5 py-0.2 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20 font-bold shrink-0">
                      {file.status_code}
                    </span>
                    <span className="text-zinc-200 truncate">{file.path}</span>
                  </div>
                  <span className="text-zinc-500 text-[11px] font-sans shrink-0 pl-2">
                    {file.description}
                  </span>
                </div>
              ))}
            </div>

            <div className="flex items-center justify-between text-xs text-zinc-400">
              <span>Choose Stash or Commit above, or proceed with acknowledgment.</span>
            </div>
          </div>
        )}

        {activeTab === 'stash' && (
          <form onSubmit={handleStashSubmit} className="space-y-3">
            <div className="space-y-1.5">
              <label className="text-xs font-medium text-zinc-300">
                Stash Message (Optional)
              </label>
              <input
                type="text"
                value={stashMessage}
                onChange={(e) => setStashMessage(e.target.value)}
                placeholder="e.g. Work in progress before task"
                disabled={isLoading}
                className="w-full px-3 py-2 rounded-lg bg-zinc-950 border border-zinc-800 text-xs text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-blue-500 font-mono"
              />
            </div>
            <p className="text-[11px] text-zinc-400">
              Runs <code className="text-zinc-300 font-mono">git stash push -u</code> to cleanly stash all modified and untracked files.
            </p>
            <button
              type="submit"
              disabled={isLoading}
              className="w-full py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-xs font-medium transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
            >
              <Archive className="w-3.5 h-3.5" />
              <span>Stash Changes & Proceed to Task</span>
            </button>
          </form>
        )}

        {activeTab === 'commit' && (
          <form onSubmit={handleCommitSubmit} className="space-y-3">
            <div className="space-y-1.5">
              <label className="text-xs font-medium text-zinc-300">
                Commit Message <span className="text-rose-400">*</span>
              </label>
              <input
                type="text"
                value={commitMessage}
                onChange={(e) => setCommitMessage(e.target.value)}
                placeholder="e.g. Save local changes before starting task"
                disabled={isLoading}
                required
                className="w-full px-3 py-2 rounded-lg bg-zinc-950 border border-zinc-800 text-xs text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-emerald-500 font-mono"
              />
            </div>
            <p className="text-[11px] text-zinc-400">
              Runs <code className="text-zinc-300 font-mono">git add -A && git commit</code> to commit all changes to your current branch.
            </p>
            <button
              type="submit"
              disabled={isLoading || !commitMessage.trim()}
              className="w-full py-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-medium transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
            >
              <GitCommit className="w-3.5 h-3.5" />
              <span>Commit Changes & Proceed to Task</span>
            </button>
          </form>
        )}

        {/* Footer actions */}
        <div className="pt-3 border-t border-zinc-800 flex items-center justify-between text-xs">
          <button
            type="button"
            onClick={onCancel}
            disabled={isLoading}
            className="px-3 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-300 font-medium transition-colors"
          >
            Cancel Task
          </button>

          <button
            type="button"
            onClick={onProceedAnyway}
            disabled={isLoading}
            title="Explicitly acknowledge: uncommitted changes will remain in primary tree and will NOT be included in task branch"
            className="px-3 py-1.5 rounded-lg bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 border border-amber-500/30 font-medium transition-colors flex items-center gap-1.5"
          >
            <span>Proceed Anyway (Explicit Acknowledgment)</span>
            <ArrowRight className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>
    </div>
  );
};
