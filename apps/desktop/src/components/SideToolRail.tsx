import { type FC, type ReactNode } from 'react';
import type { ChangeInventory as ChangeInventoryType } from '../types';

export type ActiveToolPane = 'review' | 'terminal' | 'quota' | 'files' | null;

interface SideToolRailProps {
  activePane: ActiveToolPane;
  onTogglePane: (pane: ActiveToolPane) => void;
  inventory: ChangeInventoryType | null;
  workerEventCount: number;
  children?: ReactNode;
}

export const SideToolRail: FC<SideToolRailProps> = ({
  activePane,
  onTogglePane,
  inventory,
  workerEventCount,
  children,
}) => {
  const hasChanges = Boolean(inventory && inventory.files.length > 0);

  return (
    <aside className="flex h-full border-l border-[#27272a] bg-[#09090b] shrink-0 z-10 select-none font-mono text-xs">
      {/* Expanded Side Pane */}
      {activePane && (
        <div className="w-[520px] lg:w-[620px] xl:w-[700px] flex flex-col h-full bg-[#0d0d0f] border-r border-[#27272a] overflow-hidden">
          {/* Header */}
          <div className="h-8 px-3 bg-[#121214] border-b border-[#27272a] flex items-center justify-between text-xs text-zinc-300">
            <div className="flex items-center gap-2">
              <span className="font-semibold text-zinc-200">
                {activePane === 'review'
                  ? 'review_changes'
                  : activePane === 'terminal'
                  ? 'worker_terminal'
                  : activePane === 'quota'
                  ? 'rate_limits'
                  : 'project_files'}
              </span>
              {activePane === 'review' && hasChanges && inventory && (
                <span className="text-[10px] text-zinc-400">
                  [{inventory.files.length} files: +{inventory.total_additions}/-{inventory.total_deletions}]
                </span>
              )}
            </div>

            <button
              type="button"
              onClick={() => onTogglePane(null)}
              className="text-zinc-500 hover:text-zinc-200 px-1 font-mono transition-colors"
              title="close pane"
            >
              [x]
            </button>
          </div>

          {/* Body */}
          <div className="flex-1 overflow-hidden p-2">{children}</div>
        </div>
      )}

      {/* Slim Action Rail (Matching Image 2 without colorful SVG icons) */}
      <div className="w-52 p-2 flex flex-col gap-1.5 shrink-0 bg-[#09090b]">
        <div className="text-[10px] text-zinc-500 uppercase px-2 py-1 tracking-wider">
          tools
        </div>

        {/* 1. Review */}
        <button
          type="button"
          onClick={() => onTogglePane(activePane === 'review' ? null : 'review')}
          className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded text-left transition-colors text-xs ${
            activePane === 'review'
              ? 'bg-[#27272a] text-white border border-[#3f3f46]'
              : hasChanges
              ? 'bg-[#18181b] text-zinc-100 border border-[#27272a] hover:bg-[#27272a]'
              : 'text-zinc-400 hover:text-zinc-200 hover:bg-[#18181b] border border-transparent'
          }`}
        >
          <div className="flex items-center gap-1.5">
            <span className="text-zinc-500">{hasChanges ? '[*]' : '[ ]'}</span>
            <span>review</span>
          </div>
          <span className="text-[9px] text-zinc-600">ctrl alt r</span>
        </button>

        {/* 2. Quota & Rate Limits */}
        <button
          type="button"
          onClick={() => onTogglePane(activePane === 'quota' ? null : 'quota')}
          className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded text-left transition-colors text-xs ${
            activePane === 'quota'
              ? 'bg-[#27272a] text-white border border-[#3f3f46]'
              : 'text-zinc-400 hover:text-zinc-200 hover:bg-[#18181b] border border-transparent'
          }`}
        >
          <div className="flex items-center gap-1.5">
            <span className="text-zinc-500">[~]</span>
            <span>rate limits</span>
          </div>
          <span className="text-[9px] text-zinc-600">ctrl alt b</span>
        </button>

        {/* 3. Terminal */}
        <button
          type="button"
          onClick={() => onTogglePane(activePane === 'terminal' ? null : 'terminal')}
          className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded text-left transition-colors text-xs ${
            activePane === 'terminal'
              ? 'bg-[#27272a] text-white border border-[#3f3f46]'
              : 'text-zinc-400 hover:text-zinc-200 hover:bg-[#18181b] border border-transparent'
          }`}
        >
          <div className="flex items-center gap-1.5">
            <span className="text-zinc-500">[&gt;]</span>
            <span>terminal</span>
            {workerEventCount > 0 && (
              <span className="text-[10px] text-zinc-500">({workerEventCount})</span>
            )}
          </div>
          <span className="text-[9px] text-zinc-600">ctrl alt t</span>
        </button>

        {/* 4. Files */}
        <button
          type="button"
          onClick={() => onTogglePane(activePane === 'files' ? null : 'files')}
          className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded text-left transition-colors text-xs ${
            activePane === 'files'
              ? 'bg-[#27272a] text-white border border-[#3f3f46]'
              : 'text-zinc-400 hover:text-zinc-200 hover:bg-[#18181b] border border-transparent'
          }`}
        >
          <div className="flex items-center gap-1.5">
            <span className="text-zinc-500">[/]</span>
            <span>open file</span>
          </div>
          <span className="text-[9px] text-zinc-600">ctrl alt p</span>
        </button>
      </div>
    </aside>
  );
};
