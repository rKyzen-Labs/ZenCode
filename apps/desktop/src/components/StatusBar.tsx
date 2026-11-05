import type { FC } from 'react';
import type { SystemStatus, ProjectState, QuotaUpdateEvent } from '../types';
import { GitBranch, Cpu, Brain, Activity, CircleDot, Gauge } from 'lucide-react';

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
  const getHealthColor = (health?: string) => {
    switch (health) {
      case 'Ready':
        return 'text-emerald-400';
      case 'Starting':
        return 'text-amber-400';
      case 'Degraded':
      case 'Failed':
        return 'text-rose-400';
      default:
        return 'text-[var(--foreground-muted)]';
    }
  };

  const getTaskBadge = (taskState?: string) => {
    switch (taskState) {
      case 'WorkerRunning':
        return { label: 'Worker Running', color: 'bg-blue-500/20 text-blue-400 border-blue-500/40 animate-pulse' };
      case 'Planning':
        return { label: 'Gemini Planning', color: 'bg-purple-500/20 text-purple-400 border-purple-500/40 animate-pulse' };
      case 'AwaitingReview':
        return { label: 'Awaiting Review', color: 'bg-amber-500/20 text-amber-400 border-amber-500/40' };
      case 'Complete':
        return { label: 'Task Complete', color: 'bg-emerald-500/20 text-emerald-400 border-emerald-500/40' };
      case 'Failed':
        return { label: 'Failed', color: 'bg-rose-500/20 text-rose-400 border-rose-500/40' };
      default:
        return { label: 'Idle', color: 'bg-zinc-800/60 text-zinc-400 border-zinc-700/50' };
    }
  };

  const taskBadge = getTaskBadge(status?.task_state);

  return (
    <footer className="h-7 w-full border-t border-[var(--surface-border)] bg-[var(--surface)] px-3 text-[11px] text-[var(--foreground-muted)] flex items-center justify-between select-none">
      {/* Left: Project / Git & Worktree */}
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1.5 text-zinc-300">
          <GitBranch className="w-3.5 h-3.5 text-blue-400" />
          <span>{project?.current_branch || 'no-branch'}</span>
        </div>

        <span className="text-zinc-600">|</span>

        <div className="flex items-center gap-1">
          <span className="text-zinc-400">Worktree:</span>
          <span className="text-zinc-300 font-mono text-[10px]">
            {project ? (project.git_status_summary.includes('clean') ? 'Clean' : 'Modified') : 'Detached'}
          </span>
        </div>
      </div>

      {/* Right: AI Models & Engine Health & Task status */}
      <div className="flex items-center gap-4">
        {/* Planner Model & Quota Dashboard trigger */}
        <button
          type="button"
          onClick={onOpenQuotaDashboard}
          title="Click to view real-time rate limit telemetry & waterfall cascade"
          className="flex items-center gap-1.5 text-zinc-300 hover:text-white px-1.5 py-0.5 rounded hover:bg-zinc-800 transition-colors cursor-pointer"
        >
          <Brain className="w-3.5 h-3.5 text-purple-400" />
          <span className="font-mono text-[10px] text-zinc-200">
            {quotaState?.active_model || 'gemini-3.5-flash-lite'}
          </span>
          {quotaState?.pinned_model && (
            <span className="text-[9px] px-1 rounded bg-purple-500/20 text-purple-300">
              PINNED
            </span>
          )}
          <Gauge className="w-3 h-3 text-blue-400 ml-0.5" />
        </button>

        <span className="text-zinc-600">|</span>

        {/* Requests Count */}
        <div className="flex items-center gap-1 text-zinc-400">
          <span>Reqs:</span>
          <span className="font-mono text-zinc-200">{status?.request_count ?? 0}</span>
        </div>

        <span className="text-zinc-600">|</span>

        {/* Worker Model */}
        <div className="flex items-center gap-1 text-zinc-300">
          <Cpu className="w-3.5 h-3.5 text-emerald-400" />
          <span>Worker:</span>
          <span className="font-mono text-zinc-200 text-[10px]">qwen2.5-coder:7b</span>
        </div>

        <span className="text-zinc-600">|</span>

        {/* OpenCode health */}
        <div className="flex items-center gap-1.5">
          <Activity className="w-3.5 h-3.5 text-zinc-400" />
          <span>OpenCode:</span>
          <span className={`font-medium ${getHealthColor(status?.opencode_health)}`}>
            {status?.opencode_health || 'Stopped'}
          </span>
        </div>

        <span className="text-zinc-600">|</span>

        {/* Task state badge */}
        <div className={`px-2 py-0.5 rounded border text-[10px] font-medium flex items-center gap-1 ${taskBadge.color}`}>
          <CircleDot className="w-2.5 h-2.5" />
          <span>{taskBadge.label}</span>
        </div>
      </div>
    </footer>
  );
};
