import React from 'react';
import type { ProjectState, TaskState } from '../types';
import { formatTaskState } from '../types';

interface ChatHeaderProps {
  project: ProjectState | null;
  taskState: TaskState;
  activeChatTitle: string;
  activePlannerModel: string;
  isPinned: boolean;
  onOpenQuota: () => void;
  isRightPanelOpen: boolean;
  onToggleRightPanel: () => void;
  changesCount: number;
}

export const ChatHeader: React.FC<ChatHeaderProps> = ({
  project,
  taskState,
  activeChatTitle,
  activePlannerModel,
  isPinned,
  onOpenQuota,
  isRightPanelOpen,
  onToggleRightPanel,
  changesCount,
}) => {
  const formattedState = formatTaskState(taskState);

  return (
    <header className="h-10 px-4 border-b border-zinc-800/80 bg-[#0c0c0e] flex items-center justify-between select-none font-sans shrink-0 text-[#e4e4e7] z-10">
      {/* Left: Chat Title & Context */}
      <div className="flex items-center gap-2.5 truncate">
        <span className="text-xs font-semibold text-zinc-100 truncate">
          {activeChatTitle || 'New Chat'}
        </span>

        {project && (
          <div className="flex items-center gap-1.5 text-[11px] text-zinc-500 font-mono">
            <span>/</span>
            <span className="text-zinc-400">{project.name}</span>
            <span>:</span>
            <span className="text-zinc-500">[{project.current_branch || 'main'}]</span>
          </div>
        )}

        {formattedState !== 'idle' && (
          <span className="text-[10px] px-1.5 py-0.2 rounded bg-zinc-800 text-zinc-300 font-mono uppercase tracking-wide">
            {formattedState}
          </span>
        )}
      </div>

      {/* Right Controls */}
      <div className="flex items-center gap-2">
        {/* Model Selector / Quota trigger */}
        <button
          type="button"
          onClick={onOpenQuota}
          title="Planner Model & Quota"
          className="px-2.5 py-1 rounded text-xs text-zinc-300 hover:text-white hover:bg-zinc-800/80 transition-colors font-mono border border-zinc-800 flex items-center gap-1"
        >
          <span>{isPinned ? 'pinned: 3.8-flash' : activePlannerModel}</span>
        </button>

        {/* Right Panel Toggle Button (Changes / Diff Viewer) */}
        <button
          type="button"
          onClick={onToggleRightPanel}
          className={`px-2.5 py-1 rounded text-xs font-medium transition-colors border flex items-center gap-1.5 ${
            isRightPanelOpen
              ? 'bg-zinc-800 border-zinc-700 text-white'
              : 'border-zinc-800 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40'
          }`}
        >
          <span>Changes</span>
          {changesCount > 0 && (
            <span className="px-1.5 py-0.2 rounded-full text-[10px] font-mono bg-zinc-800 text-zinc-200 border border-zinc-700">
              {changesCount}
            </span>
          )}
        </button>
      </div>
    </header>
  );
};