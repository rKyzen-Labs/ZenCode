import { type FC } from 'react';
import type { ProjectState, TaskState } from '../types';
import { isTaskState, formatTaskState } from '../types';

interface HeaderTabBarProps {
  project: ProjectState | null;
  taskState: TaskState;
  activeTab?: string;
  activePlannerModel: string;
  isPinned: boolean;
  onNewSession: () => void;
  onOpenQuota: () => void;
  onChangeProject: () => void;
  tokenCount?: string;
}

export const HeaderTabBar: FC<HeaderTabBarProps> = ({
  project,
  taskState,
  activePlannerModel,
  isPinned,
  onNewSession,
  onOpenQuota,
  onChangeProject,
  tokenCount = '8k',
}) => {
  const isRunning = isTaskState(taskState, 'WorkerRunning');
  const isPlanning = isTaskState(taskState, 'Planning');
  const isAwaitingReview = isTaskState(taskState, 'AwaitingReview');

  const statusLabel = isRunning
    ? 'running'
    : isPlanning
    ? 'planning'
    : isAwaitingReview
    ? 'review_ready'
    : formatTaskState(taskState);

  const projectName = project && typeof project.name === 'string' ? project.name.toLowerCase() : '';

  return (
    <header className="w-full bg-[#09090b] border-b border-[#27272a] select-none shrink-0 z-20 font-mono text-xs">
      {/* Top Window Bar & Tabs */}
      <div className="h-9 px-3 flex items-center justify-between border-b border-[#1f1f23]">
        {/* Left tabs */}
        <div className="flex items-center gap-2 overflow-x-auto no-scrollbar">
          <span className="text-zinc-500 font-bold px-1">[z]</span>

          {project && (
            <div className="flex items-center gap-2 px-3 py-1 bg-[#18181b] border-t border-x border-[#27272a] text-zinc-200 text-xs shrink-0 rounded-t">
              <span>worker: {projectName}</span>
              <button
                type="button"
                onClick={onNewSession}
                className="text-zinc-500 hover:text-zinc-200 transition-colors ml-1 font-mono"
                title="close session"
              >
                [x]
              </button>
            </div>
          )}

          <button
            type="button"
            onClick={onNewSession}
            className="px-2 py-0.5 text-zinc-500 hover:text-zinc-200 hover:bg-[#18181b] rounded transition-colors text-xs font-mono"
            title="new task"
          >
            [+]
          </button>
        </div>

        {/* Right Controls */}
        <div className="flex items-center gap-2">
          {project && (
            <button
              type="button"
              onClick={onChangeProject}
              className="text-[11px] text-zinc-400 hover:text-zinc-200 px-2 py-0.5 hover:bg-[#18181b] rounded border border-transparent hover:border-[#27272a] transition-colors"
            >
              [switch_repo]
            </button>
          )}

          <button
            type="button"
            onClick={onOpenQuota}
            className="text-[11px] text-zinc-300 hover:text-white px-2.5 py-0.5 bg-[#18181b] hover:bg-[#27272a] rounded border border-[#27272a] transition-colors"
          >
            {isPinned ? `pinned: 3.8-flash` : `planner: ${activePlannerModel}`}
          </button>
        </div>
      </div>

      {/* Sub-header Breadcrumb */}
      <div className="h-7 px-3 flex items-center justify-between text-[11px] text-zinc-400 bg-[#0d0d0f]">
        <div className="flex items-center gap-2 truncate">
          <span className="text-zinc-300">
            {projectName || 'zencode'}
          </span>
          <span className="text-zinc-600">/</span>
          <span className="text-zinc-400">
            {project ? `worker: ${projectName}` : 'no_project'}
          </span>
          <span className="text-zinc-600">::</span>
          <span className="text-zinc-400 font-semibold">[{statusLabel}]</span>
        </div>

        <div className="flex items-center gap-3 text-zinc-500 text-[10px]">
          <span>{tokenCount} tokens</span>
          <span>|</span>
          <span className="text-zinc-400">[wt: isolated]</span>
        </div>
      </div>
    </header>
  );
};
