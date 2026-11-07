import React, { useState } from 'react';
import type { ChangeInventory as ChangeInventoryType, MergeResult, ProjectState } from '../types';
import { ReviewPanel } from '../features/review/ReviewPanel';

interface RightPanelProps {
  inventory: ChangeInventoryType | null;
  taskId: string | null;
  onAccept: (taskId: string) => Promise<MergeResult>;
  onReject: (taskId: string) => Promise<void>;
  isLoading: boolean;
  isOpen: boolean;
  onClose: () => void;
  project: ProjectState | null;
}

export const RightPanel: React.FC<RightPanelProps> = ({
  inventory,
  taskId,
  onAccept,
  onReject,
  isLoading,
  isOpen,
  onClose,
  project,
}) => {
  const [activeTab, setActiveTab] = useState<'changes' | 'explorer'>('changes');

  if (!isOpen) return null;

  const hasChanges = Boolean(inventory && inventory.files.length > 0);

  return (
    <aside className="w-[420px] lg:w-[480px] h-full bg-[#0c0c0e] border-l border-zinc-800/80 flex flex-col font-sans select-none shrink-0 overflow-hidden text-[#e4e4e7] z-10 shadow-lg">
      {/* Top Panel Tab Bar */}
      <div className="h-10 px-3 border-b border-zinc-800/80 flex items-center justify-between bg-[#101014] shrink-0">
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => setActiveTab('changes')}
            className={`px-2.5 py-1 rounded text-xs font-medium transition-colors flex items-center gap-1.5 ${
              activeTab === 'changes'
                ? 'bg-zinc-800 text-white'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            <span>Changes</span>
            {hasChanges && inventory && (
              <span className="px-1.5 py-0.2 rounded-full text-[10px] font-mono bg-zinc-700 text-zinc-200">
                {inventory.files.length}
              </span>
            )}
          </button>

          <button
            type="button"
            onClick={() => setActiveTab('explorer')}
            className={`px-2.5 py-1 rounded text-xs font-medium transition-colors ${
              activeTab === 'explorer'
                ? 'bg-zinc-800 text-white'
                : 'text-zinc-400 hover:text-zinc-200'
            }`}
          >
            Explorer
          </button>
        </div>

        <button
          type="button"
          onClick={onClose}
          title="Close Panel"
          className="p-1 rounded text-zinc-400 hover:text-white hover:bg-zinc-800 transition-colors text-xs font-mono"
        >
          &times;
        </button>
      </div>

      {/* Main Content */}
      <div className="flex-1 overflow-hidden p-3 bg-[#09090b]">
        {activeTab === 'changes' ? (
          hasChanges ? (
            <ReviewPanel
              inventory={inventory!}
              taskId={taskId || 'active-task'}
              onAccept={onAccept}
              onReject={onReject}
              isLoading={isLoading}
            />
          ) : (
            <div className="h-full flex flex-col items-center justify-center p-6 text-center text-zinc-500 font-sans text-xs space-y-2">
              <div className="w-10 h-10 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center text-zinc-600 font-mono text-base">
                &empty;
              </div>
              <div className="font-medium text-zinc-400">No Pending Changes</div>
              <p className="text-[11px] text-zinc-600 max-w-xs leading-relaxed">
                When a local worker executes a task in its isolated worktree branch, all modified, added, and deleted files will appear here for side-by-side review.
              </p>
            </div>
          )
        ) : (
          /* Explorer Tab */
          <div className="h-full flex flex-col font-mono text-xs text-zinc-300 space-y-3 p-2 overflow-y-auto">
            <div className="space-y-1">
              <div className="text-[10px] text-zinc-500 uppercase font-bold tracking-wider font-sans">
                Project Information
              </div>
              <div className="p-2.5 bg-zinc-950 border border-zinc-800 rounded space-y-1 text-[11px]">
                <div className="text-zinc-200 font-bold truncate">
                  {project?.name || 'Untitled'}
                </div>
                <div className="text-zinc-500 text-[10px] truncate" title={project?.path}>
                  {project?.path}
                </div>
                <div className="text-zinc-400 text-[10px] pt-1 border-t border-zinc-900 flex justify-between">
                  <span>Branch:</span>
                  <span className="text-zinc-200">{project?.current_branch || 'none'}</span>
                </div>
                <div className="text-zinc-400 text-[10px] flex justify-between">
                  <span>Worktree:</span>
                  <span className={project?.is_clean ? 'text-zinc-300' : 'text-zinc-500'}>
                    {project?.git_status_summary || 'unknown'}
                  </span>
                </div>
                <div className="text-zinc-400 text-[10px] flex justify-between">
                  <span>.gitignore:</span>
                  <span className={project?.has_gitignore ? 'text-zinc-300' : 'text-zinc-500'}>
                    {project?.has_gitignore ? 'present' : 'missing'}
                  </span>
                </div>
              </div>
            </div>

            {project?.detected_editor_configs && project.detected_editor_configs.length > 0 && (
              <div className="space-y-1">
                <div className="text-[10px] text-zinc-500 uppercase font-bold tracking-wider font-sans">
                  Detected Tool Configs
                </div>
                <div className="p-2.5 bg-zinc-950 border border-zinc-800 rounded text-[11px] text-zinc-300 space-y-1 font-mono">
                  {project.detected_editor_configs.map((cfg) => (
                    <div key={cfg} className="flex items-center gap-1.5 text-zinc-400">
                      <span className="text-zinc-600">[cfg]</span>
                      <span className="text-zinc-200">{cfg}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            <div className="space-y-1">
              <div className="text-[10px] text-zinc-500 uppercase font-bold tracking-wider font-sans">
                Git Isolation Invariants
              </div>
              <div className="p-2.5 bg-zinc-950 border border-zinc-800 rounded text-[11px] text-zinc-400 space-y-1.5 font-sans leading-relaxed">
                <div>&bull; Every task operates on a dedicated branch in an isolated worktree.</div>
                <div>&bull; Primary branch is never modified by local workers.</div>
                <div>&bull; Human review gate is mandatory prior to merge.</div>
              </div>
            </div>
          </div>
        )}
      </div>
    </aside>
  );
};