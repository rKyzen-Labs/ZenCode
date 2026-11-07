import React from 'react';
import type { ChangeInventory, ExecutionResult } from '../types';

export type TaskSummaryStatus =
  | 'Completed'
  | 'Completed with warnings'
  | 'Failed'
  | 'Stuck (needs attention)';

interface TaskSummaryProps {
  inventory: ChangeInventory | null;
  verificationRuns: ExecutionResult[];
  workerSummary: string | null;
  isFailed?: boolean;
  isStuck?: boolean;
  onTriggerCommand?: (program: string, args: string[], cwd?: string, description?: string) => void;
  onViewDiff?: () => void;
  onViewLogs?: () => void;
}

export const TaskSummary: React.FC<TaskSummaryProps> = ({
  inventory,
  verificationRuns,
  workerSummary,
  isFailed = false,
  isStuck = false,
  onTriggerCommand,
  onViewDiff,
  onViewLogs,
}) => {
  // Determine overall status
  let status: TaskSummaryStatus = 'Completed';
  if (isStuck) {
    status = 'Stuck (needs attention)';
  } else if (isFailed) {
    status = 'Failed';
  } else if (
    verificationRuns.some(
      (r) => !r.success || (r.exit_code !== null && r.exit_code !== 0) || r.cancelled
    )
  ) {
    status = 'Completed with warnings';
  }

  const statusBadgeStyle = {
    Completed: 'border-emerald-600 bg-emerald-950/40 text-emerald-300',
    'Completed with warnings': 'border-amber-600 bg-amber-950/40 text-amber-300',
    Failed: 'border-red-600 bg-red-950/40 text-red-300',
    'Stuck (needs attention)': 'border-red-600 bg-red-950/50 text-red-200 animate-pulse',
  }[status];

  // Group files by category
  const modifiedFiles = inventory?.files.filter((f) => f.category === 'Modified') || [];
  const addedFiles =
    inventory?.files.filter((f) => f.category === 'Added' || f.category === 'Untracked') || [];
  const deletedFiles = inventory?.files.filter((f) => f.category === 'Deleted') || [];

  return (
    <div className="bg-[#121214] border border-[#27272a] rounded p-4 space-y-3 font-mono text-xs">
      {/* Header bar with Status Badge */}
      <div className="flex items-center justify-between border-b border-[#27272a] pb-2.5">
        <div className="flex items-center gap-2">
          <span className="text-zinc-500 font-bold">[task_summary]</span>
          <span
            className={`px-2 py-0.5 rounded text-[11px] font-bold border uppercase tracking-wider ${statusBadgeStyle}`}
          >
            [{status}]
          </span>
        </div>

        {inventory && (
          <div className="flex items-center gap-2 text-[11px] text-zinc-400">
            <span>branch:</span>
            <span className="text-zinc-200 underline">{inventory.branch_name}</span>
          </div>
        )}
      </div>

      {/* Natural language summary from worker's final response */}
      {workerSummary ? (
        <div className="space-y-1">
          <div className="text-[10px] text-zinc-500 uppercase tracking-wider font-semibold">
            worker report:
          </div>
          <div className="p-2.5 bg-[#09090b] border border-[#1f1f23] text-zinc-200 text-xs leading-relaxed whitespace-pre-wrap rounded">
            {workerSummary}
          </div>
        </div>
      ) : (
        <div className="text-zinc-500 italic text-[11px]">
          Worker finished implementation steps in the isolated worktree.
        </div>
      )}

      {/* Files Changed Breakdown */}
      {inventory && (
        <div className="space-y-1.5 pt-1">
          <div className="flex items-center justify-between text-[10px] text-zinc-500 uppercase tracking-wider font-semibold">
            <span>
              file changes ({inventory.files.length} total: +{inventory.total_additions} / -
              {inventory.total_deletions})
            </span>
            {onViewDiff && (
              <button
                type="button"
                onClick={onViewDiff}
                className="text-zinc-400 hover:text-zinc-200 lowercase underline"
              >
                [view full diff &gt;]
              </button>
            )}
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-3 gap-2 text-[11px]">
            {/* Modified */}
            <div className="p-2 bg-[#09090b] border border-[#1f1f23] rounded space-y-1">
              <div className="flex items-center justify-between">
                <span className="text-amber-400 font-semibold">[M] modified</span>
                <span className="text-zinc-400 font-bold">{modifiedFiles.length}</span>
              </div>
              <div className="text-[10px] text-zinc-500 space-y-0.5 max-h-16 overflow-y-auto">
                {modifiedFiles.length === 0 ? (
                  <span className="text-zinc-600">none</span>
                ) : (
                  modifiedFiles.map((f, i) => (
                    <div key={i} className="truncate" title={f.path}>
                      {f.path}
                    </div>
                  ))
                )}
              </div>
            </div>

            {/* Added */}
            <div className="p-2 bg-[#09090b] border border-[#1f1f23] rounded space-y-1">
              <div className="flex items-center justify-between">
                <span className="text-emerald-400 font-semibold">[A] added</span>
                <span className="text-zinc-400 font-bold">{addedFiles.length}</span>
              </div>
              <div className="text-[10px] text-zinc-500 space-y-0.5 max-h-16 overflow-y-auto">
                {addedFiles.length === 0 ? (
                  <span className="text-zinc-600">none</span>
                ) : (
                  addedFiles.map((f, i) => (
                    <div key={i} className="truncate" title={f.path}>
                      {f.path}
                    </div>
                  ))
                )}
              </div>
            </div>

            {/* Deleted */}
            <div className="p-2 bg-[#09090b] border border-[#1f1f23] rounded space-y-1">
              <div className="flex items-center justify-between">
                <span className="text-rose-400 font-semibold">[D] deleted</span>
                <span className="text-zinc-400 font-bold">{deletedFiles.length}</span>
              </div>
              <div className="text-[10px] text-zinc-500 space-y-0.5 max-h-16 overflow-y-auto">
                {deletedFiles.length === 0 ? (
                  <span className="text-zinc-600">none</span>
                ) : (
                  deletedFiles.map((f, i) => (
                    <div key={i} className="truncate" title={f.path}>
                      {f.path}
                    </div>
                  ))
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Verification State */}
      <div className="pt-2 border-t border-[#1f1f23] space-y-1.5">
        <div className="flex items-center justify-between text-[10px] text-zinc-500 uppercase tracking-wider font-semibold">
          <span>verification checks</span>
          {onViewLogs && (
            <button
              type="button"
              onClick={onViewLogs}
              className="text-zinc-500 hover:text-zinc-300 lowercase"
            >
              [worker logs &gt;]
            </button>
          )}
        </div>

        {verificationRuns.length === 0 ? (
          <div className="flex items-center justify-between p-2 bg-[#09090b] border border-[#1f1f23] rounded text-[11px] text-zinc-500">
            <span>[pending]: No verification commands executed yet.</span>
            {onTriggerCommand && inventory && (
              <div className="flex items-center gap-1.5">
                <button
                  type="button"
                  onClick={() =>
                    onTriggerCommand(
                      'cargo',
                      ['check', '-p', 'zencode-core'],
                      inventory.worktree_path,
                      'Verify compilation of task changes'
                    )
                  }
                  className="px-2 py-0.5 border border-zinc-700 hover:border-zinc-500 text-zinc-300 hover:text-white rounded text-[10px] transition-colors"
                >
                  [run cargo check]
                </button>
                <button
                  type="button"
                  onClick={() =>
                    onTriggerCommand(
                      'cargo',
                      ['test', '--workspace'],
                      inventory.worktree_path,
                      'Run workspace test suite'
                    )
                  }
                  className="px-2 py-0.5 border border-zinc-700 hover:border-zinc-500 text-zinc-300 hover:text-white rounded text-[10px] transition-colors"
                >
                  [run cargo test]
                </button>
              </div>
            )}
          </div>
        ) : (
          <div className="space-y-1">
            {verificationRuns.map((run, idx) => (
              <div
                key={idx}
                className="flex items-center justify-between p-1.5 bg-[#09090b] border border-[#1f1f23] rounded text-[11px]"
              >
                <div className="flex items-center gap-2 font-mono">
                  <span className="text-zinc-500">$</span>
                  <span className="text-zinc-300 font-semibold">{run.program}</span>
                  <span className="text-zinc-500">[{run.args.join(' ')}]</span>
                </div>
                <div>
                  {run.cancelled ? (
                    <span className="text-amber-500 font-bold">[cancelled]</span>
                  ) : run.success || run.exit_code === 0 ? (
                    <span className="text-emerald-400 font-bold">[exit: 0 ok]</span>
                  ) : (
                    <span className="text-rose-400 font-bold">
                      [exit: {run.exit_code ?? 'err'} fail]
                    </span>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
