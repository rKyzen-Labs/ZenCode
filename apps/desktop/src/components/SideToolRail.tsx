import { type FC, type ReactNode } from 'react';
import {
  GitCompare,
  Globe,
  Terminal,
  FolderOpen,
  X,
} from 'lucide-react';
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
  const hasChanges = inventory && inventory.files.length > 0;

  return (
    <aside className="flex h-full border-l border-[#1f1f23] bg-[#0d0d0f] shrink-0 z-10 select-none">
      {/* Expanded Pane (if a tool is active) */}
      {activePane && (
        <div className="w-[520px] lg:w-[640px] xl:w-[720px] flex flex-col h-full bg-[#121215] border-r border-[#1f1f23] overflow-hidden shadow-2xl">
          {/* Pane Top Tab Header (matches Image 2: "New tab x +") */}
          <div className="h-9 px-3 bg-[#18181b] border-b border-[#27272a] flex items-center justify-between text-xs font-medium text-zinc-300">
            <div className="flex items-center gap-2">
              <span className="font-semibold text-zinc-200 capitalize">
                {activePane === 'review'
                  ? 'Review Changes'
                  : activePane === 'terminal'
                  ? 'Worker Terminal'
                  : activePane === 'quota'
                  ? 'Rate Limit Quotas'
                  : 'Project Files'}
              </span>
              {activePane === 'review' && hasChanges && (
                <span className="text-[10px] font-mono px-1.5 py-0.2 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                  {inventory.files.length} file{inventory.files.length !== 1 ? 's' : ''} (+{inventory.total_additions} -{inventory.total_deletions})
                </span>
              )}
            </div>

            <div className="flex items-center gap-1 text-zinc-400">
              <button
                type="button"
                onClick={() => onTogglePane(null)}
                className="p-1 rounded hover:bg-zinc-800 hover:text-zinc-200 transition-colors"
                title="Close side pane"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          {/* Pane Content */}
          <div className="flex-1 overflow-hidden p-3">{children}</div>
        </div>
      )}

      {/* Slim Action Button Rail (Identical to Image 2) */}
      <div className="w-56 p-3 flex flex-col gap-2 shrink-0 bg-[#0d0d0f]">
        <div className="text-[10px] font-semibold text-zinc-500 uppercase tracking-wider px-2 py-1">
          Quick Tools
        </div>

        {/* 1. Review [CTRL ALT R] */}
        <button
          type="button"
          onClick={() => onTogglePane(activePane === 'review' ? null : 'review')}
          className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-all ${
            activePane === 'review'
              ? 'bg-blue-600/20 text-blue-300 border border-blue-500/40'
              : hasChanges
              ? 'bg-emerald-500/10 text-emerald-300 border border-emerald-500/30 hover:bg-emerald-500/20'
              : 'bg-zinc-900/60 hover:bg-zinc-800 text-zinc-300 border border-zinc-800/80'
          }`}
          title="Review Git Changes (Ctrl+Alt+R)"
        >
          <div className="flex items-center gap-2">
            <GitCompare className="w-4 h-4 text-emerald-400 shrink-0" />
            <span>Review</span>
            {hasChanges && (
              <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
            )}
          </div>
          <span className="text-[9px] font-mono text-zinc-500">CTRL ALT R</span>
        </button>

        {/* 2. Browser / Quota [CTRL ALT B] */}
        <button
          type="button"
          onClick={() => onTogglePane(activePane === 'quota' ? null : 'quota')}
          className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-all ${
            activePane === 'quota'
              ? 'bg-blue-600/20 text-blue-300 border border-blue-500/40'
              : 'bg-zinc-900/60 hover:bg-zinc-800 text-zinc-300 border border-zinc-800/80'
          }`}
          title="Quota Telemetry (Ctrl+Alt+B)"
        >
          <div className="flex items-center gap-2">
            <Globe className="w-4 h-4 text-blue-400 shrink-0" />
            <span>Browser</span>
          </div>
          <span className="text-[9px] font-mono text-zinc-500">CTRL ALT B</span>
        </button>

        {/* 3. Terminal [CTRL ALT T] */}
        <button
          type="button"
          onClick={() => onTogglePane(activePane === 'terminal' ? null : 'terminal')}
          className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-all ${
            activePane === 'terminal'
              ? 'bg-blue-600/20 text-blue-300 border border-blue-500/40'
              : 'bg-zinc-900/60 hover:bg-zinc-800 text-zinc-300 border border-zinc-800/80'
          }`}
          title="Worker Stream & Logs (Ctrl+Alt+T)"
        >
          <div className="flex items-center gap-2">
            <Terminal className="w-4 h-4 text-amber-400 shrink-0" />
            <span>Terminal</span>
            {workerEventCount > 0 && (
              <span className="text-[9px] font-mono px-1 rounded bg-zinc-800 text-zinc-400">
                {workerEventCount}
              </span>
            )}
          </div>
          <span className="text-[9px] font-mono text-zinc-500">CTRL ALT T</span>
        </button>

        {/* 4. Open file [CTRL ALT P] */}
        <button
          type="button"
          onClick={() => onTogglePane(activePane === 'files' ? null : 'files')}
          className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-all ${
            activePane === 'files'
              ? 'bg-blue-600/20 text-blue-300 border border-blue-500/40'
              : 'bg-zinc-900/60 hover:bg-zinc-800 text-zinc-300 border border-zinc-800/80'
          }`}
          title="Inspect Files (Ctrl+Alt+P)"
        >
          <div className="flex items-center gap-2">
            <FolderOpen className="w-4 h-4 text-purple-400 shrink-0" />
            <span>Open file</span>
          </div>
          <span className="text-[9px] font-mono text-zinc-500">CTRL ALT P</span>
        </button>
      </div>
    </aside>
  );
};
