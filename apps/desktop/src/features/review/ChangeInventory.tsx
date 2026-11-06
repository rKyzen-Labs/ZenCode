import { useState, type FC } from 'react';
import type { ChangeInventory as ChangeInventoryType, FileChange, ChangeCategory } from '../../types';

interface ChangeInventoryProps {
  inventory: ChangeInventoryType;
  selectedFile: FileChange | null;
  onSelectFile: (file: FileChange) => void;
}

const CATEGORY_META: Record<string, { label: string; badge: string }> = {
  Modified: { label: 'modified', badge: 'M' },
  Added: { label: 'added', badge: 'A' },
  Untracked: { label: 'untracked', badge: 'U' },
  Deleted: { label: 'deleted', badge: 'D' },
  Renamed: { label: 'renamed', badge: 'R' },
  Copied: { label: 'copied', badge: 'C' },
  Binary: { label: 'binary', badge: 'BIN' },
  ModeChange: { label: 'mode_change', badge: 'CHMOD' },
  Symlink: { label: 'symlink', badge: 'LINK' },
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
    <div className="w-64 flex flex-col h-full bg-[#09090b] border border-[#27272a] rounded overflow-hidden shrink-0 font-mono text-xs">
      {/* Header */}
      <div className="p-2.5 bg-[#121214] border-b border-[#27272a] space-y-1.5 shrink-0">
        <div className="flex items-center justify-between text-zinc-300">
          <span className="font-semibold text-zinc-100">[files]</span>
          <span className="text-[11px] text-zinc-500">
            {inventory.files.length} total
          </span>
        </div>

        <div className="flex items-center justify-between text-[11px] text-zinc-500 pt-1 border-t border-[#1f1f23]">
          <span className="truncate max-w-[120px]">{inventory.branch_name}</span>
          <span>
            +{inventory.total_additions} / -{inventory.total_deletions}
          </span>
        </div>
      </div>

      {/* File List */}
      <div className="flex-1 overflow-y-auto p-1.5 space-y-2">
        {categoryOrder.map((category) => {
          const files = groups[category];
          if (!files || files.length === 0) return null;

          const meta = CATEGORY_META[category] || { label: category.toLowerCase(), badge: '?' };
          const isCollapsed = collapsedGroups[category];

          return (
            <div key={category} className="space-y-0.5">
              <button
                type="button"
                onClick={() => toggleGroup(category)}
                className="w-full flex items-center justify-between px-1.5 py-1 text-[11px] text-zinc-400 hover:text-zinc-200 transition-colors text-left"
              >
                <span>
                  {isCollapsed ? '[+]' : '[-]'} {meta.label} ({files.length})
                </span>
              </button>

              {!isCollapsed && (
                <div className="space-y-0.5 pl-2">
                  {files.map((file) => {
                    const isSelected = selectedFile?.path === file.path;

                    return (
                      <button
                        key={file.path}
                        type="button"
                        onClick={() => onSelectFile(file)}
                        className={`w-full flex items-center justify-between px-2 py-1 rounded text-left transition-colors text-[11px] ${
                          isSelected
                            ? 'bg-zinc-200 text-black font-medium'
                            : 'text-zinc-400 hover:text-zinc-100 hover:bg-[#18181b]'
                        }`}
                      >
                        <div className="flex items-center gap-1.5 truncate pr-1">
                          <span className="text-[10px] text-zinc-500">[{meta.badge}]</span>
                          <span className="truncate" title={file.path}>
                            {file.path}
                          </span>
                        </div>

                        <div className="text-[10px] shrink-0 text-zinc-500">
                          {file.is_binary ? (
                            <span>bin</span>
                          ) : (
                            <span>+{file.additions}/-{file.deletions}</span>
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
