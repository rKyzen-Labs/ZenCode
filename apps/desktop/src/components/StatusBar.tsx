import type { FC } from 'react';
import type { SystemStatus, ProjectState, QuotaUpdateEvent } from '../types';
import { formatEngineHealth, formatTaskState } from '../types';

interface StatusBarProps {
  project: ProjectState | null;
  status: SystemStatus | null;
  quotaState?: QuotaUpdateEvent | null;
  onOpenQuotaDashboard?: () => void;
}

export const StatusBar: FC<StatusBarProps> = ({
  project,
  status,
  quotaState,
  onOpenQuotaDashboard,
}) => {
  const formattedTaskState = formatTaskState(status?.task_state);
  const formattedOpencodeHealth = formatEngineHealth(status?.opencode_health);

  return (
    <footer className="h-6 w-full border-t border-[#27272a] bg-[#09090b] px-3 text-[10px] text-zinc-500 font-mono flex items-center justify-between select-none">
      {/* Left: Project / Git branch & worktree */}
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1.5 text-zinc-400">
          <span>[git: {project?.current_branch || 'none'}]</span>
        </div>

        <span className="text-zinc-700">|</span>

        <div className="flex items-center gap-1">
          <span>
            [wt:{' '}
            {project
              ? project.git_status_summary?.includes('clean')
                ? 'clean'
                : 'dirty'
              : 'none'}
            ]
          </span>
        </div>
      </div>

      {/* Right: AI Models & Engine Health & Task status */}
      <div className="flex items-center gap-3">
        {/* Planner Model & Quota Trigger */}
        <button
          type="button"
          onClick={onOpenQuotaDashboard}
          title="rate limit telemetry & waterfall cascade"
          className="flex items-center gap-1 text-zinc-400 hover:text-zinc-100 transition-colors"
        >
          <span>[planner: {quotaState?.active_model || 'gemini-3.5-flash-lite'}]</span>
          {quotaState?.pinned_model && (
            <span className="text-zinc-200 border border-[#3f3f46] px-1 rounded text-[9px]">
              PINNED
            </span>
          )}
        </button>

        <span className="text-zinc-700">|</span>

        {/* Requests Count */}
        <div className="flex items-center gap-1 text-zinc-500">
          <span>[reqs: {status?.request_count ?? 0}]</span>
        </div>

        <span className="text-zinc-700">|</span>

        {/* Worker Model */}
        <div className="flex items-center gap-1 text-zinc-400">
          <span>[worker: granite4.1:8b]</span>
        </div>

        <span className="text-zinc-700">|</span>

        {/* OpenCode health */}
        <div className="flex items-center gap-1 text-zinc-400">
          <span>[opencode: {formattedOpencodeHealth}]</span>
        </div>

        <span className="text-zinc-700">|</span>

        {/* Task state badge */}
        <div className="text-zinc-300">
          <span>[state: {formattedTaskState}]</span>
        </div>
      </div>
    </footer>
  );
};
