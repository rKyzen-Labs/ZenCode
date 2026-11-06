import { useState, type FC } from 'react';
import type { ChangeInventory as ChangeInventoryType, FileChange, ChangeCategory } from '../../types';
import {
  Layers,
  ChevronDown,
  ChevronRight,
  GitBranch,
} from 'lucide-react';

interface ChangeInventoryProps {
  inventory: ChangeInventoryType;
  selectedFile: FileChange | null;
  onSelectFile: (file: FileChange) => void;
}

const CATEGORY_META: Record<
  string,
  { label: string; badge: string; color: string; badgeBg: string; borderColor: string }
> = {
  Modified: {
    label: 'Modified Files',
    badge: 'MOD',
    color: 'text-amber-400',
    badgeBg: 'bg-amber-500/10 text-amber-300',
    borderColor: 'border-amber-500/30',
  },
  Added: {
    label: 'Added Files',
    badge: 'ADD',
    color: 'text-emerald-400',
    badgeBg: 'bg-emerald-500/10 text-emerald-300',
    borderColor: 'border-emerald-500/30',
  },
  Untracked: {
    label: 'Untracked Files',
    badge: 'NEW',
    color: 'text-cyan-400',
    badgeBg: 'bg-cyan-500/10 text-cyan-300',
    borderColor: 'border-cyan-500/30',
  },
  Deleted: {
    label: 'Deleted Files',
    badge: 'DEL',
    color: 'text-rose-400',
    badgeBg: 'bg-rose-500/10 text-rose-300',
    borderColor: 'border-rose-500/30',
  },
  Renamed: {
    label: 'Renamed / Moved',
    badge: 'REN',
    color: 'text-purple-400',
    badgeBg: 'bg-purple-500/10 text-purple-300',
    borderColor: 'border-purple-500/30',
  },
  Copied: {
    label: 'Copied',
    badge: 'CPY',
    color: 'text-indigo-400',
    badgeBg: 'bg-indigo-500/10 text-indigo-300',
    borderColor: 'border-indigo-500/30',
  },
  Binary: {
    label: 'Binary Files',
    badge: 'BIN',
    color: 'text-violet-400',
    badgeBg: 'bg-violet-500/10 text-violet-300',
    borderColor: 'border-violet-500/30',
  },
  ModeChange: {
    label: 'File Mode Changes',
    badge: 'MODE',
    color: 'text-orange-400',
    badgeBg: 'bg-orange-500/10 text-orange-300',
    borderColor: 'border-orange-500/30',
  },
  Symlink: {
    label: 'Symlinks',
    badge: 'LINK',
    color: 'text-teal-400',
    badgeBg: 'bg-teal-500/10 text-teal-300',
    borderColor: 'border-teal-500/30',
  },
};

export const ChangeInventory: FC<ChangeInventoryProps> = ({
  inventory,
  selectedFile,
  onSelectFile,
}) => {
  const [collapsedGroups, setCollapsedGroups] = useState<Record<string, boolean>>({});

  const toggleGroup = (cat: string) => {
    setCollapsedGroups((prev) => ({ ...prev, [cat]: !prev[cat] }));
  };

  // Group files by category
  const groups = inventory.files.reduce((acc, file) => {
    const key = file.category;
    if (!acc[key]) acc[key] = [];
    acc[key].push(file);
    return acc;
  }, {} as Record<ChangeCategory, FileChange[]>);

  const categoryOrder: ChangeCategory[] = [
    'Modified',
    'Added',
    'Untracked',
    'Deleted',
    'Renamed',
    'Copied',
    'Binary',
    'ModeChange',
    'Symlink',
  ];

  return (
    <div className="w-80 flex flex-col h-full bg-zinc-950 border border-zinc-800 rounded-lg overflow-hidden shrink-0">
      {/* Header with aggregate stats */}
      <div className="p-3 bg-zinc-900/90 border-b border-zinc-800 shrink-0 space-y-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Layers className="w-4 h-4 text-zinc-400" />
            <h3 className="text-xs font-semibold text-zinc-200">Change Inventory</h3>
          </div>
          <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-zinc-800 text-zinc-400 border border-zinc-700">
            {inventory.files.length} file{inventory.files.length !== 1 ? 's' : ''}
          </span>
        </div>

        <div className="flex items-center justify-between text-[11px] font-mono pt-1 border-t border-zinc-800/80">
          <div className="flex items-center gap-2 text-zinc-400">
            <GitBranch className="w-3.5 h-3.5 text-blue-400" />
            <span className="truncate max-w-[130px]">{inventory.branch_name}</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-emerald-400 font-medium">+{inventory.total_additions}</span>
            <span className="text-rose-400 font-medium">-{inventory.total_deletions}</span>
          </div>
        </div>
      </div>

      {/* File list grouped by category */}
      <div className="flex-1 overflow-y-auto p-2 space-y-3">
        {categoryOrder.map((category) => {
          const files = groups[category];
          if (!files || files.length === 0) return null;

          const meta = CATEGORY_META[category] || {
            label: category,
            badge: category.substring(0, 3).toUpperCase(),
            color: 'text-zinc-400',
            badgeBg: 'bg-zinc-800 text-zinc-300',
            borderColor: 'border-zinc-700',
          };

          const isCollapsed = collapsedGroups[category];

          return (
            <div key={category} className="space-y-1">
              {/* Category group header */}
              <button
                type="button"
                onClick={() => toggleGroup(category)}
                className="w-full flex items-center justify-between px-2 py-1 text-[11px] font-semibold text-zinc-400 hover:text-zinc-200 rounded transition-colors text-left"
              >
                <div className="flex items-center gap-1.5">
                  {isCollapsed ? (
                    <ChevronRight className="w-3 h-3 text-zinc-500" />
                  ) : (
                    <ChevronDown className="w-3 h-3 text-zinc-500" />
                  )}
                  <span>{meta.label}</span>
                </div>
                <span className={`text-[9px] font-mono px-1.5 py-0.2 rounded border ${meta.badgeBg} ${meta.borderColor}`}>
                  {files.length}
                </span>
              </button>

              {/* Files in this category */}
              {!isCollapsed && (
                <div className="space-y-0.5 pl-2">
                  {files.map((file) => {
                    const isSelected = selectedFile?.path === file.path;

                    return (
                      <button
                        key={file.path}
                        type="button"
                        onClick={() => onSelectFile(file)}
                        className={`w-full flex items-center justify-between px-2 py-1.5 rounded text-left transition-colors text-xs font-mono group ${
                          isSelected
                            ? 'bg-blue-600/20 border border-blue-500/50 text-blue-200'
                            : 'hover:bg-zinc-900 border border-transparent text-zinc-300'
                        }`}
                      >
                        <div className="flex items-center gap-2 truncate min-w-0 pr-1">
                          <span
                            className={`text-[9px] px-1 py-0.2 rounded border shrink-0 font-bold ${meta.badgeBg} ${meta.borderColor}`}
                          >
                            {meta.badge}
                          </span>
                          <span className="truncate" title={file.path}>
                            {file.path}
                          </span>
                        </div>

                        <div className="flex items-center gap-1 shrink-0 text-[10px]">
                          {file.is_binary ? (
                            <span className="text-[9px] text-zinc-500">bin</span>
                          ) : (
                            <>
                              {file.additions > 0 && (
                                <span className="text-emerald-400">+{file.additions}</span>
                              )}
                              {file.deletions > 0 && (
                                <span className="text-rose-400">-{file.deletions}</span>
                              )}
                            </>
                          )}
                        </div>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};
