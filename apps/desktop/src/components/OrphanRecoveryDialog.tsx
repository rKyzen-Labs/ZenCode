import { type FC } from 'react';
import type { OrphanWorktree } from '../types';

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
    <div className="fixed inset-0 z-50 bg-black/85 flex items-center justify-center p-4 font-mono text-xs">
      <div className="bg-[#121214] border border-[#3f3f46] rounded max-w-xl w-full p-5 space-y-4 shadow-2xl">
        {/* Header */}
        <div className="flex items-start justify-between border-b border-[#27272a] pb-3">
          <div className="space-y-0.5">
            <div className="flex items-center gap-2">
              <span className="font-semibold text-zinc-100">[orphaned_worktree_recovery]</span>
              <span className="text-[10px] text-zinc-400 border border-[#27272a] px-1.5 rounded">
                {orphans.length} found
              </span>
            </div>
            <p className="text-[11px] text-zinc-500">
              Unmerged worktree(s) detected from a previous session. Zencode never silently deletes orphaned worktrees.
            </p>
          </div>

          <button
            type="button"
            onClick={onIgnore}
            disabled={isLoading}
            className="text-zinc-500 hover:text-zinc-200"
          >
            [x]
          </button>
        </div>

        {/* Orphan List */}
        <div className="max-h-60 overflow-y-auto space-y-2">
          {orphans.map((orphan) => (
            <div
              key={orphan.task_id}
              className="p-2.5 rounded bg-[#09090b] border border-[#27272a] flex items-center justify-between"
            >
              <div className="space-y-0.5 truncate pr-2">
                <div className="flex items-center gap-2">
                  <span className="text-zinc-200 font-semibold truncate">
                    {orphan.branch_name}
                  </span>
                  <span className="text-[10px] text-zinc-500">
                    ({formatAge(orphan.age_seconds)})
                  </span>
                </div>
                <div className="text-[10px] text-zinc-600 truncate" title={orphan.worktree_path}>
                  {orphan.worktree_path}
                </div>
              </div>

              <div className="flex items-center gap-1.5 shrink-0">
                <button
                  type="button"
                  onClick={() => onReview(orphan.task_id)}
                  disabled={isLoading}
                  className="px-2 py-0.5 rounded bg-[#18181b] hover:bg-[#27272a] text-zinc-300 hover:text-white border border-[#27272a] text-[11px] transition-colors disabled:opacity-40"
                >
                  [review]
                </button>

                <button
                  type="button"
                  onClick={() => onReject(orphan.task_id)}
                  disabled={isLoading}
                  className="px-2 py-0.5 rounded bg-[#18181b] hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200 border border-[#27272a] text-[11px] transition-colors disabled:opacity-40"
                >
                  [reject]
                </button>
              </div>
            </div>
          ))}
        </div>

        {/* Footer */}
        <div className="pt-2 border-t border-[#27272a] flex items-center justify-between">
          <span className="text-[10px] text-zinc-600">
            * reject will delete worktree folder and task branch cleanly
          </span>
          <button
            type="button"
            onClick={onIgnore}
            disabled={isLoading}
            className="px-3 py-1 rounded bg-[#18181b] hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200 text-xs"
          >
            [ignore_for_now]
          </button>
        </div>
      </div>
    </div>
  );
};
