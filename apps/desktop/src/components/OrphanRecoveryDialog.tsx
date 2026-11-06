import { type FC } from 'react';
import type { OrphanWorktree } from '../types';
import {
  GitBranch,
  Clock,
  Eye,
  Trash2,
  X,
  History,
} from 'lucide-react';

interface OrphanRecoveryDialogProps {
  orphans: OrphanWorktree[];
  onReview: (taskId: string) => Promise<void>;
  onReject: (taskId: string) => Promise<void>;
  onIgnore: () => void;
  isLoading: boolean;
}

function formatAge(seconds: number): string {
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

export const OrphanRecoveryDialog: FC<OrphanRecoveryDialogProps> = ({
  orphans,
  onReview,
  onReject,
  onIgnore,
  isLoading,
}) => {
  if (orphans.length === 0) return null;

  return (
    <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-zinc-900 border border-blue-500/40 rounded-xl max-w-xl w-full p-6 shadow-2xl space-y-4">
        {/* Header */}
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-full bg-blue-500/10 border border-blue-500/30 flex items-center justify-center text-blue-400 shrink-0">
              <History className="w-5 h-5" />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-zinc-100">
                Orphaned Worktree Recovery
              </h3>
              <p className="text-xs text-zinc-400">
                {orphans.length} unmerged task worktree(s) detected from previous sessions
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={onIgnore}
            disabled={isLoading}
            className="text-zinc-500 hover:text-zinc-300 transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <p className="text-xs text-zinc-300 leading-relaxed">
          Zencode never silently deletes orphaned worktrees. You can review the proposed changes before deciding to merge or discard them.
        </p>

        {/* Orphan List */}
        <div className="max-h-64 overflow-y-auto space-y-2">
          {orphans.map((orphan) => (
            <div
              key={orphan.task_id}
              className="p-3 rounded-lg bg-zinc-950 border border-zinc-800 flex items-center justify-between text-xs font-mono"
            >
              <div className="space-y-1 truncate pr-2">
                <div className="flex items-center gap-2">
                  <GitBranch className="w-3.5 h-3.5 text-blue-400 shrink-0" />
                  <span className="text-zinc-200 font-semibold truncate">
                    {orphan.branch_name}
                  </span>
                  <span className="text-[10px] text-zinc-500 font-sans flex items-center gap-1 shrink-0">
                    <Clock className="w-3 h-3" />
                    {formatAge(orphan.age_seconds)}
                  </span>
                </div>
                <div className="text-[11px] text-zinc-500 truncate" title={orphan.worktree_path}>
                  {orphan.worktree_path}
                </div>
              </div>

              <div className="flex items-center gap-1.5 shrink-0 font-sans">
                <button
                  type="button"
                  onClick={() => onReview(orphan.task_id)}
                  disabled={isLoading}
                  className="px-2.5 py-1 rounded bg-blue-600/20 hover:bg-blue-600/30 text-blue-300 border border-blue-500/40 text-[11px] font-medium flex items-center gap-1 transition-colors disabled:opacity-50"
                >
                  <Eye className="w-3 h-3" />
                  <span>Review</span>
                </button>

                <button
                  type="button"
                  onClick={() => onReject(orphan.task_id)}
                  disabled={isLoading}
                  className="px-2.5 py-1 rounded bg-rose-500/10 hover:bg-rose-500/20 text-rose-400 border border-rose-500/30 text-[11px] font-medium flex items-center gap-1 transition-colors disabled:opacity-50"
                >
                  <Trash2 className="w-3 h-3" />
                  <span>Reject</span>
                </button>
              </div>
            </div>
          ))}
        </div>

        {/* Footer */}
        <div className="pt-2 border-t border-zinc-800 flex items-center justify-end">
          <button
            type="button"
            onClick={onIgnore}
            disabled={isLoading}
            className="px-4 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-xs text-zinc-300 font-medium transition-colors"
          >
            Ignore for Now
          </button>
        </div>
      </div>
    </div>
  );
};
