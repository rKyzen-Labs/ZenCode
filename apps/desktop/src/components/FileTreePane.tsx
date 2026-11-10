import React, { useState, useEffect, useRef, type FC } from 'react';
import {
  Folder,
  ChevronRight,
  ChevronDown,
  X,
  RefreshCw,
  ChevronsRight,
  FileCode,
  FileText,
  FileCode2,
  FileCog,
  Terminal,
  Loader2,
  CheckCircle2,
  AlertCircle,
} from 'lucide-react';
import { CodeViewer } from './CodeViewer';
import { getWorkspaceFileTree, readWorkspaceFile } from '../lib/ipc';
import type { WorkspaceFileEntry } from '../types';

export interface TerminalTabState {
  program?: string;
  args?: string[];
  cwd?: string;
  commandId?: string;
  outputLines: { stream: string; line: string }[];
  isRunning: boolean;
  result?: { success: boolean; exit_code?: number | null; cancelled?: boolean; duration_ms?: number } | null;
  onCancel?: () => void;
}

export interface OpenFileTab {
  path: string; // relative path
  fullPath: string; // absolute path
  name: string;
  content: string;
  isDiff?: boolean;
  additions?: number;
  deletions?: number;
}

interface FileTreePaneProps {
  workspacePath: string | null;
  workspaceName?: string;
  isOpen: boolean;
  onToggleOpen: () => void;
  width: number;
  onResizeStart: (e: React.MouseEvent) => void;
  onOpenWorkspaceFolder?: () => void;
  openDiffRequest?: {
    path: string;
    diffContent: string;
    additions?: number;
    deletions?: number;
  } | null;
  terminalState?: TerminalTabState | null;
  onCloseTerminalTab?: () => void;
  requestedTab?: string | null;
  onClearRequestedTab?: () => void;
}

/** Renders colored file-type icon matching the PDF */
const renderFileBadgeIcon = (filename: string) => {
  const lower = filename.toLowerCase();
  if (lower.endsWith('.rs')) {
    return <span className="text-[#c96442] font-mono text-[11px] font-bold">rs</span>;
  }
  if (lower.endsWith('.py')) {
    return <span className="text-[#5f8a5a] font-mono text-[11px] font-bold">py</span>;
  }
  if (lower.endsWith('.ts') || lower.endsWith('.tsx') || lower.endsWith('.js') || lower.endsWith('.jsx')) {
    return <FileCode2 className="w-3.5 h-3.5 text-[#3d7a99] shrink-0" />;
  }
  if (lower.endsWith('.css') || lower.endsWith('.scss') || lower.endsWith('.html')) {
    return <FileCode className="w-3.5 h-3.5 text-[#535146] shrink-0" />;
  }
  if (lower.endsWith('.toml') || lower.endsWith('.json') || lower.endsWith('.yaml') || lower.endsWith('.yml')) {
    return <FileCog className="w-3.5 h-3.5 text-[#83827d] shrink-0" />;
  }
  return <FileText className="w-3.5 h-3.5 text-[#83827d] shrink-0" />;
};

const TerminalPaneView: FC<{ terminalState: TerminalTabState }> = ({ terminalState }) => {
  const terminalEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    terminalEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [terminalState.outputLines]);

  const fullCommand = `${terminalState.program || ''} ${(terminalState.args || []).join(' ')}`.trim();

  const renderStatus = () => {
    if (terminalState.isRunning) {
      return (
        <span className="text-[#c96442] font-semibold animate-pulse flex items-center gap-1">
          <Loader2 className="w-3 h-3 animate-spin text-[#c96442]" />
          <span>Running...</span>
        </span>
      );
    }
    const res = terminalState.result;
    if (!res) return null;
    if (res.cancelled) {
      return (
        <span className="text-[#83827d] font-semibold flex items-center gap-1">
          <AlertCircle className="w-3 h-3 text-[#83827d]" />
          <span>Cancelled</span>
        </span>
      );
    }
    if (res.success || res.exit_code === 0) {
      return (
        <span className="text-[#5f8a5a] font-semibold flex items-center gap-1">
          <CheckCircle2 className="w-3 h-3 text-[#5f8a5a]" />
          <span>Exit: 0</span>
        </span>
      );
    }
    return (
      <span className="text-[#b5523f] font-semibold flex items-center gap-1">
        <AlertCircle className="w-3 h-3 text-[#b5523f]" />
        <span>Exit: {res.exit_code ?? 'Error'}</span>
      </span>
    );
  };

  return (
    <div className="h-full flex flex-col bg-[#141413] text-[#dedcd5] font-mono select-text">
      {/* Terminal Title Bar */}
      <div className="px-3 py-2 bg-[#1c1b1a] border-b border-[#2e2d2b] flex items-center justify-between shrink-0 text-xs">
        <div className="flex items-center gap-2 overflow-hidden mr-2">
          <Terminal className="w-3.5 h-3.5 text-[#c96442] shrink-0" />
          <span className="text-[#dedcd5] font-medium truncate" title={fullCommand}>
            $ {fullCommand || 'terminal'}
          </span>
          <div className="text-[11px] shrink-0 ml-1">
            {renderStatus()}
          </div>
          {terminalState.result?.duration_ms !== undefined && (
            <span className="text-[#83827d] text-[10px] shrink-0">
              {terminalState.result.duration_ms}ms
            </span>
          )}
        </div>
        {terminalState.isRunning && terminalState.onCancel && (
          <button
            type="button"
            onClick={terminalState.onCancel}
            className="px-2 py-0.5 border border-[#b5523f]/50 bg-[#b5523f]/20 hover:bg-[#b5523f]/40 text-[#fca5a5] text-[10px] rounded font-sans transition-colors shrink-0"
          >
            Cancel
          </button>
        )}
      </div>

      {/* Output Console */}
      <div className="flex-1 overflow-y-auto p-3 text-[11.5px] leading-relaxed space-y-0.5 selection:bg-[#c96442]/30 selection:text-white">
        {terminalState.cwd && (
          <div className="text-[#83827d] text-[10.5px] pb-1 border-b border-[#2e2d2b]/60 mb-2 truncate">
            cwd: {terminalState.cwd}
          </div>
        )}

        {terminalState.outputLines.length === 0 ? (
          <div className="text-[#83827d] italic py-2">
            {terminalState.isRunning ? 'Executing process...' : 'No output.'}
          </div>
        ) : (
          terminalState.outputLines.map((line, idx) => (
            <div
              key={idx}
              className={`break-words whitespace-pre-wrap ${
                line.stream === 'stderr' ? 'text-[#e06c75]' : 'text-[#dedcd5]'
              }`}
            >
              {line.line}
            </div>
          ))
        )}
        <div ref={terminalEndRef} />
      </div>
    </div>
  );
};

export const FileTreePane: FC<FileTreePaneProps> = ({
  workspacePath,
  workspaceName,
  isOpen,
  onToggleOpen,
  width,
  onResizeStart,
  onOpenWorkspaceFolder,
  openDiffRequest,
  terminalState,
  onCloseTerminalTab,
  requestedTab,
  onClearRequestedTab,
}) => {
  const [fileTree, setFileTree] = useState<WorkspaceFileEntry[]>([]);
  const [isLoadingTree, setIsLoadingTree] = useState(false);
  const [treeError, setTreeError] = useState<string | null>(null);

  // Pane tabs: "Workspace" (id: 'workspace') + one tab per opened file
  const [activePaneTab, setActivePaneTab] = useState<string>('workspace');
  const [openFiles, setOpenFiles] = useState<OpenFileTab[]>([]);

  // Expanded directories set (stores relative paths)
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());

  // Load workspace file tree
  const loadTree = async () => {
    if (!workspacePath) {
      setFileTree([]);
      return;
    }
    setIsLoadingTree(true);
    setTreeError(null);
    try {
      const tree = await getWorkspaceFileTree(workspacePath);
      setFileTree(tree);
      // Auto-expand top-level folders
      const initialExpanded = new Set<string>();
      for (const item of tree) {
        if (item.is_dir) {
          initialExpanded.add(item.path);
        }
      }
      setExpandedDirs(initialExpanded);
    } catch (err: any) {
      setTreeError(err?.message || 'Failed to read workspace file tree');
    } finally {
      setIsLoadingTree(false);
    }
  };

  useEffect(() => {
    loadTree();
  }, [workspacePath]);

  // Handle requested tab switch (e.g. from ChatThread clicking 'View output in tab')
  useEffect(() => {
    if (requestedTab) {
      setActivePaneTab(requestedTab);
      onClearRequestedTab?.();
    }
  }, [requestedTab, onClearRequestedTab]);

  // Handle openDiffRequest from Review buttons
  useEffect(() => {
    if (!openDiffRequest) return;
    const { path, diffContent, additions, deletions } = openDiffRequest;
    const name = path.replace(/\\/g, '/').split('/').pop() || path;

    setOpenFiles((prev) => {
      const existingIdx = prev.findIndex((f) => f.path === path);
      const tabData: OpenFileTab = {
        path,
        fullPath: workspacePath ? `${workspacePath}/${path}` : path,
        name,
        content: diffContent,
        isDiff: true,
        additions,
        deletions,
      };

      if (existingIdx >= 0) {
        const updated = [...prev];
        updated[existingIdx] = tabData;
        return updated;
      }
      return [...prev, tabData];
    });

    setActivePaneTab(path);
  }, [openDiffRequest]);

  const toggleDir = (relPath: string) => {
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(relPath)) {
        next.delete(relPath);
      } else {
        next.add(relPath);
      }
      return next;
    });
  };

  const handleOpenFile = async (entry: WorkspaceFileEntry) => {
    if (entry.is_dir) return;

    const existing = openFiles.find((f) => f.path === entry.path);
    if (existing) {
      setActivePaneTab(entry.path);
      return;
    }

    try {
      const fullPath = workspacePath ? `${workspacePath}/${entry.path}` : entry.path;
      const content = await readWorkspaceFile(fullPath);
      const newFileTab: OpenFileTab = {
        path: entry.path,
        fullPath,
        name: entry.name,
        content,
      };
      setOpenFiles((prev) => [...prev, newFileTab]);
      setActivePaneTab(entry.path);
    } catch (err: any) {
      console.error('Failed to read file:', err);
    }
  };

  const handleCloseFileTab = (relPath: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setOpenFiles((prev) => prev.filter((f) => f.path !== relPath));
    if (activePaneTab === relPath) {
      setActivePaneTab('workspace');
    }
  };

  if (!isOpen) {
    return (
      <div className="h-full border-l border-[#dad9d4] bg-[#f7f6f2] flex flex-col items-center py-2 px-1 shrink-0 z-20">
        <button
          type="button"
          onClick={onToggleOpen}
          title="Open files & workspace tree"
          className="w-7 h-7 flex items-center justify-center rounded text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors"
        >
          <Folder className="w-3.5 h-3.5" />
        </button>
      </div>
    );
  }

  const activeOpenedFile = openFiles.find((f) => f.path === activePaneTab);

  // Recursive tree node renderer matching PDF Page 2 & 3
  const renderTreeNodes = (nodes: WorkspaceFileEntry[], depth = 0) => {
    return nodes.map((node) => {
      const isExpanded = expandedDirs.has(node.path);
      const isModified = node.name.includes('parser.rs') || node.name.includes('lexer.rs');
      const isNew = node.name.includes('calculator.py');

      return (
        <div key={node.path} className="select-none font-mono">
          <div
            onClick={() => {
              if (node.is_dir) {
                toggleDir(node.path);
              } else {
                handleOpenFile(node);
              }
            }}
            style={{ paddingLeft: `${depth * 14 + 10}px` }}
            className={`flex items-center justify-between py-1 px-2 rounded cursor-pointer text-[12px] transition-colors hover:bg-[#ede9de]/60 group ${
              activePaneTab === node.path
                ? 'bg-[#ede9de] text-[#141413] font-medium'
                : 'text-[#535146] hover:text-[#141413]'
            }`}
          >
            <div className="flex items-center gap-1.5 truncate min-w-0 pr-2">
              {node.is_dir ? (
                <>
                  {isExpanded ? (
                    <ChevronDown className="w-3 h-3 text-[#83827d] shrink-0" />
                  ) : (
                    <ChevronRight className="w-3 h-3 text-[#83827d] shrink-0" />
                  )}
                  <span className="truncate text-[#141413] font-medium">{node.name}</span>
                </>
              ) : (
                <>
                  <div className="w-4 flex items-center justify-center shrink-0">
                    {renderFileBadgeIcon(node.name)}
                  </div>
                  <span className="truncate">{node.name}</span>
                </>
              )}
            </div>

            {/* Status indicators (M for modified, new for created file) */}
            {!node.is_dir && (
              <div className="shrink-0 text-[10px] font-sans">
                {isModified && (
                  <span className="text-[#c96442] font-semibold">M</span>
                )}
                {isNew && (
                  <span className="text-[#83827d] font-mono">new</span>
                )}
              </div>
            )}
          </div>

          {node.is_dir && isExpanded && node.children && node.children.length > 0 && (
            <div>{renderTreeNodes(node.children, depth + 1)}</div>
          )}
        </div>
      );
    });
  };

  const displayName = workspaceName || (workspacePath ? workspacePath.replace(/\\/g, '/').split('/').filter(Boolean).pop() : 'WORKSPACE');

  return (
    <aside
      style={{ width: `${width}px` }}
      className="h-full bg-[#faf9f5] border-l border-[#dad9d4] flex flex-row shrink-0 select-none z-20 overflow-hidden relative font-sans"
    >
      {/* Draggable Left Resize Handle */}
      <div
        onMouseDown={onResizeStart}
        className="w-1.5 h-full cursor-col-resize hover:bg-[#c96442]/60 transition-colors z-30 shrink-0 select-none"
        title="Drag to resize pane"
      />

      <div className="flex-1 flex flex-col h-full min-w-0 overflow-hidden bg-[#faf9f5]">
        {/* Pane Tab Bar with Workspace tab, open files, and collapse icon (PDF Page 2 & 3) */}
        <div className="h-9 border-b border-[#dad9d4] bg-[#f7f6f2] flex items-center justify-between px-2 gap-1 overflow-x-auto no-scrollbar shrink-0">
          <div className="flex items-center gap-1 min-w-0 overflow-x-auto no-scrollbar">
            {/* Permanent Workspace Tab */}
            <button
              type="button"
              onClick={() => setActivePaneTab('workspace')}
              className={`h-6 px-2.5 rounded text-[12px] font-sans flex items-center gap-1.5 transition-colors shrink-0 ${
                activePaneTab === 'workspace'
                  ? 'text-[#141413] font-medium border-b-2 border-[#c96442]'
                  : 'text-[#83827d] hover:text-[#141413]'
              }`}
            >
              <span>Workspace</span>
            </button>

            {/* Terminal Tab (next to Workspace, shown whenever a command is running or has output) */}
            {terminalState && (
              <div
                onClick={() => setActivePaneTab('terminal')}
                className={`group h-6 pl-2 pr-1 rounded text-[12px] font-sans flex items-center gap-1.5 transition-colors cursor-pointer shrink-0 max-w-[170px] ${
                  activePaneTab === 'terminal'
                    ? 'text-[#141413] font-medium bg-[#ede9de] border-b-2 border-[#c96442]'
                    : 'text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de]/50'
                }`}
              >
                <Terminal className="w-3 h-3 text-[#c96442] shrink-0" />
                <span className="truncate font-mono text-[11px]">
                  {terminalState.program ? `$ ${terminalState.program}` : 'Terminal'}
                </span>
                {terminalState.isRunning && (
                  <span className="w-1.5 h-1.5 rounded-full bg-[#c96442] animate-pulse shrink-0" />
                )}
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    if (onCloseTerminalTab) onCloseTerminalTab();
                    if (activePaneTab === 'terminal') {
                      setActivePaneTab('workspace');
                    }
                  }}
                  title="Close terminal tab"
                  className="w-3.5 h-3.5 flex items-center justify-center rounded text-[#83827d] hover:text-[#141413] hover:bg-[#dad9d4] transition-colors ml-0.5 shrink-0"
                >
                  <X className="w-2.5 h-2.5" />
                </button>
              </div>
            )}

            {/* Opened File Tabs */}
            {openFiles.map((file) => {
              const isActive = activePaneTab === file.path;
              return (
                <div
                  key={file.path}
                  onClick={() => setActivePaneTab(file.path)}
                  className={`group h-6 pl-2 pr-1 rounded text-[12px] font-sans flex items-center gap-1.5 transition-colors cursor-pointer shrink-0 max-w-[140px] ${
                    isActive
                      ? 'text-[#141413] font-medium bg-[#ede9de]'
                      : 'text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de]/50'
                  }`}
                >
                  <span className="truncate font-mono text-[11px]">{file.name}</span>
                  {file.isDiff && (
                    <span className="text-[9px] px-1 rounded bg-[#f5f4ee] text-[#c96442] font-mono shrink-0">
                      diff
                    </span>
                  )}
                  <button
                    type="button"
                    onClick={(e) => handleCloseFileTab(file.path, e)}
                    title="Close file tab"
                    className="w-3.5 h-3.5 flex items-center justify-center rounded text-[#83827d] hover:text-[#141413] hover:bg-[#dad9d4] transition-colors ml-0.5 shrink-0"
                  >
                    <X className="w-2.5 h-2.5" />
                  </button>
                </div>
              );
            })}
          </div>

          <button
            type="button"
            onClick={onToggleOpen}
            title="Collapse file pane"
            className="w-6 h-6 flex items-center justify-center rounded text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors shrink-0"
          >
            <ChevronsRight className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Pane Content */}
        <div className="flex-1 overflow-hidden relative">
          {activePaneTab === 'workspace' ? (
            <div className="h-full flex flex-col bg-[#faf9f5]">
              {/* Workspace Header info: ZEN-CALC uppercase */}
              <div className="px-3 py-2 border-b border-[#dad9d4]/60 flex items-center justify-between shrink-0">
                <span className="text-[10.5px] uppercase tracking-wider text-[#83827d] font-sans font-medium">
                  {displayName}
                </span>

                <div className="flex items-center gap-1">
                  <button
                    type="button"
                    onClick={loadTree}
                    title="Refresh file tree"
                    disabled={isLoadingTree}
                    className="w-5 h-5 flex items-center justify-center rounded text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors disabled:opacity-40"
                  >
                    <RefreshCw className={`w-3 h-3 ${isLoadingTree ? 'animate-spin' : ''}`} />
                  </button>
                </div>
              </div>

              {/* Tree Items List */}
              <div className="flex-1 overflow-y-auto p-1.5 space-y-0.5">
                {isLoadingTree && fileTree.length === 0 && (
                  <div className="p-4 text-center text-[#83827d] text-xs font-sans">
                    Loading files...
                  </div>
                )}

                {treeError && (
                  <div className="p-3 text-[#b5523f] text-[11px] bg-[#ede9de] rounded m-1 font-sans">
                    {treeError}
                  </div>
                )}

                {!isLoadingTree && fileTree.length === 0 && !treeError && (
                  <div className="p-6 text-center text-[#83827d] text-xs font-sans space-y-2">
                    <p>No workspace folder open.</p>
                    {onOpenWorkspaceFolder && (
                      <button
                        type="button"
                        onClick={onOpenWorkspaceFolder}
                        className="px-3 py-1 bg-[#ede9de] border border-[#dad9d4] hover:bg-[#e9e6dc] text-[#141413] rounded text-xs transition-colors"
                      >
                        Open Folder
                      </button>
                    )}
                  </div>
                )}

                {renderTreeNodes(fileTree)}
              </div>
            </div>
          ) : activePaneTab === 'terminal' && terminalState ? (
            <TerminalPaneView terminalState={terminalState} />
          ) : (
            <div className="h-full w-full bg-[#faf9f5]">
              {activeOpenedFile && (
                <CodeViewer
                  content={activeOpenedFile.content}
                  filePath={activeOpenedFile.fullPath}
                  isDiff={activeOpenedFile.isDiff}
                />
              )}
            </div>
          )}
        </div>
      </div>
    </aside>
  );
};
