import { useState, useRef, useEffect, type FC } from 'react';
import {
  Folder,
  ChevronDown,
  FolderPlus,
  Check,
} from 'lucide-react';
import { listWorkspaces, pickFolder } from '../lib/ipc';
import type { ProjectState, WorkspaceRecord } from '../types';

interface WorkspaceSelectorRowProps {
  currentProject: ProjectState | null;
  onOpenWorkspace: (path: string) => void;
  workerType?: 'local' | 'groq';
  className?: string;
}

export const WorkspaceSelectorRow: FC<WorkspaceSelectorRowProps> = ({
  currentProject,
  onOpenWorkspace,
  workerType: _workerType,
  className = '',
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const [workspaces, setWorkspaces] = useState<WorkspaceRecord[]>([]);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen]);

  const handleOpenDropdown = async () => {
    try {
      const records = await listWorkspaces();
      setWorkspaces(records || []);
    } catch (e) {
      console.warn('Could not list workspaces:', e);
    }
    setIsOpen((prev) => !prev);
  };

  const handleBrowseFolder = async () => {
    setIsOpen(false);
    try {
      const selected = await pickFolder();
      if (selected && selected.trim()) {
        onOpenWorkspace(selected.trim());
      }
    } catch (err) {
      console.error('Failed to pick folder:', err);
    }
  };

  const workspaceName = currentProject ? currentProject.name : 'Select Workspace';
  const workspacePath = currentProject ? currentProject.path : 'Click to choose directory';

  return (
    <div ref={dropdownRef} className={`relative select-none font-sans ${className}`}>
      {/* Workspace Bar: Matching PDF Page 1 [Folder icon] [name] [path] [Chevron] */}
      <div
        onClick={handleOpenDropdown}
        className="w-full flex items-center justify-between px-3 py-2 text-xs bg-[#f5f4ee] border border-[#dad9d4] hover:border-[#83827d]/50 transition-colors rounded-lg cursor-pointer group shadow-sm"
      >
        <div className="flex items-center gap-2 truncate min-w-0">
          <Folder className="w-3.5 h-3.5 text-[#83827d] shrink-0" />
          <span className="text-[#141413] font-medium text-[12px] truncate">
            {workspaceName}
          </span>
          <span className="text-[#83827d] font-mono text-[11px] truncate">
            {workspacePath}
          </span>
        </div>

        <ChevronDown className="w-3.5 h-3.5 text-[#83827d] group-hover:text-[#141413] shrink-0 ml-2" />
      </div>

      {/* Dropdown Menu */}
      {isOpen && (
        <div className="absolute left-0 right-0 top-11 bg-[#faf9f5] border border-[#dad9d4] rounded-lg shadow-xl z-50 p-1.5 space-y-1">
          <div className="px-2 py-1 text-[10px] font-sans font-medium text-[#83827d] uppercase tracking-wider">
            Switch Workspace
          </div>

          <div className="max-h-48 overflow-y-auto space-y-0.5">
            {workspaces.map((ws) => {
              const isSelected = currentProject?.path === ws.path;
              return (
                <div
                  key={ws.path}
                  onClick={() => {
                    onOpenWorkspace(ws.path);
                    setIsOpen(false);
                  }}
                  className={`flex items-center justify-between px-2.5 py-1.5 rounded cursor-pointer text-xs transition-colors ${
                    isSelected
                      ? 'bg-[#ede9de] text-[#141413] font-medium'
                      : 'text-[#535146] hover:bg-[#f5f4ee] hover:text-[#141413]'
                  }`}
                >
                  <div className="flex items-center gap-2 truncate">
                    <Folder className="w-3.5 h-3.5 text-[#83827d] shrink-0" />
                    <span className="truncate text-[12px]">{ws.name}</span>
                    <span className="text-[10px] text-[#83827d] font-mono truncate">{ws.path}</span>
                  </div>
                  {isSelected && <Check className="w-3.5 h-3.5 text-[#c96442] shrink-0 ml-2" />}
                </div>
              );
            })}
          </div>

          <div className="pt-1 border-t border-[#dad9d4]">
            <button
              type="button"
              onClick={handleBrowseFolder}
              className="w-full flex items-center gap-2 px-2.5 py-1.5 text-xs text-[#141413] hover:bg-[#ede9de] rounded transition-colors font-sans"
            >
              <FolderPlus className="w-3.5 h-3.5 text-[#c96442]" />
              <span>Open Other Folder…</span>
            </button>
          </div>
        </div>
      )}
    </div>
  );
};
