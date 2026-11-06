import { type FC } from 'react';
import type { ProjectState, TaskState } from '../types';
import {
  Menu,
  LayoutGrid,
  Search,
  X,
  Plus,
  Sparkles,
  Gauge,
  Shield,
  FolderGit2,
} from 'lucide-react';

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
  activeTab: _activeTab,
  activePlannerModel,
  isPinned,
  onNewSession,
  onOpenQuota,
  onChangeProject,
  tokenCount = '8k',
}) => {
  const isWorkerRunning = taskState === 'WorkerRunning';
  const isPlanning = taskState === 'Planning';
  const isAwaitingReview = taskState === 'AwaitingReview';

  return (
    <div className="w-full bg-[#0d0d0f] border-b border-[#1f1f23] select-none shrink-0 z-20 flex flex-col">
      {/* Top Window / Tab Bar */}
      <div className="h-10 px-3 flex items-center justify-between border-b border-[#1f1f23]/60">
        {/* Left window actions & Session Tabs */}
        <div className="flex items-center gap-2 overflow-x-auto no-scrollbar">
          <div className="flex items-center gap-1 text-zinc-500 mr-2 shrink-0">
            <button
              type="button"
              className="p-1 rounded hover:bg-zinc-800/60 hover:text-zinc-300 transition-colors"
              title="Menu"
            >
              <Menu className="w-4 h-4" />
            </button>
            <button
              type="button"
              className="p-1 rounded hover:bg-zinc-800/60 hover:text-zinc-300 transition-colors"
              title="Layout"
            >
              <LayoutGrid className="w-4 h-4" />
            </button>
            <button
              type="button"
              className="p-1 rounded hover:bg-zinc-800/60 hover:text-zinc-300 transition-colors"
              title="Search"
            >
              <Search className="w-4 h-4" />
            </button>
          </div>

          {/* Active Session Tab (matches Example Image 2) */}
          {project && (
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-t-lg bg-[#18181b] border-t border-x border-[#27272a] text-xs font-medium text-zinc-200 shrink-0 shadow-sm">
              <div className="w-4 h-4 rounded bg-blue-600 flex items-center justify-center text-white text-[9px] font-bold">
                Z
              </div>
              <span className="truncate max-w-[150px]">
                Worker: {project.name}
              </span>
              <button
                type="button"
                onClick={onNewSession}
                className="text-zinc-500 hover:text-zinc-300 ml-1 rounded hover:bg-zinc-800 p-0.5"
                title="Close session"
              >
                <X className="w-3 h-3" />
              </button>
            </div>
          )}

          {/* New Tab Button */}
          <button
            type="button"
            onClick={onNewSession}
            className="p-1 text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800/60 rounded transition-colors shrink-0"
            title="New Session"
          >
            <Plus className="w-4 h-4" />
          </button>
        </div>

        {/* Right header controls */}
        <div className="flex items-center gap-2 shrink-0">
          {project && (
            <button
              type="button"
              onClick={onChangeProject}
              className="text-[11px] px-2 py-0.5 rounded text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/50 flex items-center gap-1 transition-colors font-mono"
            >
              <FolderGit2 className="w-3.5 h-3.5 text-zinc-500" />
              <span>{project.name}</span>
            </button>
          )}

          {/* Quota & Model Pill */}
          <button
            type="button"
            onClick={onOpenQuota}
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded border text-[11px] font-medium transition-colors ${
              isPinned
                ? 'bg-purple-500/10 text-purple-300 border-purple-500/30 hover:bg-purple-500/20'
                : 'bg-zinc-900 text-zinc-300 border-zinc-800 hover:bg-zinc-800'
            }`}
            title="View Real-Time Rate Limits"
          >
            <Sparkles className="w-3 h-3 text-purple-400" />
            <span className="font-mono text-[10px]">
              {isPinned ? `Pinned: 3.8 Flash` : activePlannerModel}
            </span>
            <Gauge className="w-3 h-3 text-zinc-400" />
          </button>
        </div>
      </div>

      {/* Sub-header / Breadcrumbs (matching Example Image 2) */}
      <div className="h-8 px-4 flex items-center justify-between text-xs text-zinc-400 bg-[#121215]/50">
        <div className="flex items-center gap-2 truncate">
          <div className="w-3.5 h-3.5 rounded bg-blue-600/80 flex items-center justify-center text-white text-[8px] font-bold">
            Z
          </div>
          <span className="text-zinc-300 font-medium">{project ? project.name : 'ZenCode'}</span>
          <span className="text-zinc-600">/</span>
          <span className="text-zinc-400 truncate">
            {project ? `Worker: ${project.name}` : 'Ready'}
          </span>

          {isWorkerRunning && (
            <span className="ml-2 inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-400 border border-blue-500/30 text-[10px] font-mono">
              <span className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse" />
              Worker Running
            </span>
          )}

          {isPlanning && (
            <span className="ml-2 inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-purple-500/10 text-purple-400 border border-purple-500/30 text-[10px] font-mono">
              <span className="w-1.5 h-1.5 rounded-full bg-purple-400 animate-pulse" />
              Planning
            </span>
          )}

          {isAwaitingReview && (
            <span className="ml-2 inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 text-[10px] font-mono">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
              Changes Ready for Review
            </span>
          )}
        </div>

        {/* Right token counter badge */}
        <div className="flex items-center gap-3 text-zinc-500 font-mono text-[11px]">
          <span className="px-1.5 py-0.5 rounded bg-zinc-900 border border-zinc-800 text-zinc-400 text-[10px]">
            {tokenCount} tokens
          </span>
          <div className="flex items-center gap-1 text-[11px] text-zinc-400">
            <Shield className="w-3.5 h-3.5 text-emerald-400" />
            <span className="text-[10px] hidden sm:inline">Git Worktree Active</span>
          </div>
        </div>
      </div>
    </div>
  );
};
