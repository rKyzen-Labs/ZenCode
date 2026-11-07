import React, { useState } from 'react';
import type { ProjectState, SystemStatus, QuotaUpdateEvent } from '../types';
import { formatEngineHealth } from '../types';

export interface StoredChatSummary {
  id: string; // task_id
  workspace_path: string;
  title: string;
  created_at: string;
  status: 'active' | 'review' | 'merged' | 'rejected' | 'failed' | 'idle';
  files_count?: number;
}

interface SidebarProps {
  currentWorkspace: ProjectState | null;
  workspaces: string[];
  chats: StoredChatSummary[];
  activeChatId: string | null;
  onSelectChat: (chatId: string) => void;
  onNewChat: () => void;
  onSwitchWorkspace: (path: string) => void;
  onOpenProjectDialog: () => void;
  status: SystemStatus | null;
  quotaState: QuotaUpdateEvent | null;
  onOpenQuota: () => void;
  onDeleteChat?: (chatId: string) => void;
  isCollapsed: boolean;
  onToggleCollapse: () => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  currentWorkspace,
  workspaces,
  chats,
  activeChatId,
  onSelectChat,
  onNewChat,
  onSwitchWorkspace,
  onOpenProjectDialog,
  status,
  quotaState,
  onOpenQuota,
  onDeleteChat,
  isCollapsed,
  onToggleCollapse,
}) => {
  const [isWorkspaceMenuOpen, setIsWorkspaceMenuOpen] = useState(false);
  const [isChatsTreeExpanded, setIsChatsTreeExpanded] = useState(true);

  if (isCollapsed) {
    return (
      <div className="w-12 h-full bg-[#0c0c0e] border-r border-zinc-800/80 flex flex-col items-center py-3 select-none font-sans shrink-0">
        <button
          type="button"
          onClick={onToggleCollapse}
          title="Expand Sidebar"
          className="w-8 h-8 flex items-center justify-center rounded text-zinc-400 hover:text-white hover:bg-zinc-800/60 transition-colors"
        >
          &rarr;
        </button>

        <div className="my-3 w-6 border-t border-zinc-800" />

        <button
          type="button"
          onClick={onNewChat}
          disabled={!currentWorkspace}
          title="New Chat / Task"
          className="w-8 h-8 flex items-center justify-center rounded text-zinc-300 hover:text-white hover:bg-zinc-800 transition-colors disabled:opacity-30"
        >
          +
        </button>

        <div className="mt-auto flex flex-col gap-3 items-center">
          {/* Quiet Status Dot */}
          <div
            className={`w-2 h-2 rounded-full ${
              status?.ollama_ready ? 'bg-emerald-400' : 'bg-amber-500'
            }`}
            title="Local Engine Status"
          />
        </div>
      </div>
    );
  }

  const isGeminiReady =
    status?.gemini_ready ?? Boolean(status?.gemini_status?.includes('Configured'));
  const isOllamaReady =
    status?.ollama_ready ??
    Boolean(status?.ollama_status?.includes('Available') || status?.ollama_status?.includes('ready'));
  const formattedOpencode = formatEngineHealth(status?.opencode_health);
  const isOpencodeReady = formattedOpencode === 'ready';

  const statusBadge = (s: StoredChatSummary['status']) => {
    switch (s) {
      case 'active':
        return <span className="text-[9px] px-1 rounded bg-blue-950/80 text-blue-300 border border-blue-800/50">running</span>;
      case 'review':
        return <span className="text-[9px] px-1 rounded bg-amber-950/80 text-amber-300 border border-amber-800/50">diff ready</span>;
      case 'merged':
        return <span className="text-[9px] px-1 rounded bg-emerald-950/80 text-emerald-300 border border-emerald-800/50">merged</span>;
      case 'rejected':
        return <span className="text-[9px] px-1 rounded bg-zinc-800 text-zinc-400 border border-zinc-700">rejected</span>;
      case 'failed':
        return <span className="text-[9px] px-1 rounded bg-red-950/80 text-red-300 border border-red-800/50">failed</span>;
      default:
        return null;
    }
  };

  return (
    <aside className="w-64 h-full bg-[#0c0c0e] border-r border-zinc-800/80 flex flex-col select-none font-sans shrink-0 overflow-hidden text-[#e4e4e7]">
      {/* Top Workspace Selector Header */}
      <div className="p-2.5 border-b border-zinc-800/80 relative">
        <div className="flex items-center justify-between gap-1">
          <button
            type="button"
            onClick={() => setIsWorkspaceMenuOpen(!isWorkspaceMenuOpen)}
            className="flex-1 flex items-center justify-between px-2 py-1.5 rounded-md hover:bg-zinc-800/60 transition-colors text-left group overflow-hidden"
          >
            <div className="truncate">
              <div className="text-[10px] text-zinc-500 font-medium tracking-wider uppercase">
                Workspace
              </div>
              <div className="text-xs font-semibold text-zinc-200 group-hover:text-white truncate">
                {currentWorkspace?.name || 'No Project Opened'}
              </div>
            </div>
            <span className="text-zinc-500 text-xs ml-1">&#x25BE;</span>
          </button>

          <button
            type="button"
            onClick={onToggleCollapse}
            title="Collapse Sidebar"
            className="p-1 rounded text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800/60 transition-colors"
          >
            &larr;
          </button>
        </div>

        {/* Dropdown Menu for Workspaces */}
        {isWorkspaceMenuOpen && (
          <div className="absolute top-full left-2 right-2 mt-1 z-30 bg-zinc-900 border border-zinc-800 rounded-lg shadow-xl p-1.5 space-y-1">
            <button
              type="button"
              onClick={() => {
                setIsWorkspaceMenuOpen(false);
                onOpenProjectDialog();
              }}
              className="w-full px-2.5 py-1.5 rounded hover:bg-zinc-800 text-left text-xs font-medium text-zinc-200 flex items-center justify-between"
            >
              <span>Open Folder...</span>
              <span className="text-zinc-500 font-mono text-[10px]">[+]</span>
            </button>

            {workspaces.length > 0 && (
              <>
                <div className="px-2 pt-1 pb-0.5 text-[10px] font-medium text-zinc-500 uppercase tracking-wider">
                  Switch Workspace
                </div>
                <div className="max-h-40 overflow-y-auto space-y-0.5">
                  {workspaces.map((path) => {
                    const name = path.split(/[/\\]/).filter(Boolean).pop() || path;
                    const isCurrent = currentWorkspace?.path === path;
                    return (
                      <button
                        key={path}
                        type="button"
                        onClick={() => {
                          setIsWorkspaceMenuOpen(false);
                          if (!isCurrent) onSwitchWorkspace(path);
                        }}
                        className={`w-full px-2 py-1 rounded text-left text-xs truncate transition-colors flex items-center justify-between ${
                          isCurrent
                            ? 'bg-zinc-800 text-white font-medium'
                            : 'text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-200'
                        }`}
                      >
                        <span className="truncate">{name}</span>
                        {isCurrent && <span className="text-[10px] text-zinc-400 ml-1">&bull;</span>}
                      </button>
                    );
                  })}
                </div>
              </>
            )}
          </div>
        )}
      </div>

      {/* Primary Action Button: New Chat */}
      <div className="p-2.5 border-b border-zinc-800/60">
        <button
          type="button"
          onClick={onNewChat}
          disabled={!currentWorkspace}
          className="w-full py-1.5 px-3 rounded-md bg-zinc-800/80 hover:bg-zinc-700/80 text-zinc-100 font-medium text-xs flex items-center justify-between transition-colors border border-zinc-700/60 disabled:opacity-40"
        >
          <span>New Chat</span>
          <span className="font-mono text-xs text-zinc-400">+</span>
        </button>
      </div>

      {/* Tree Section: Projects -> Chats/Tasks */}
      <div className="flex-1 overflow-y-auto p-2 space-y-3">
        {currentWorkspace ? (
          <div className="space-y-1">
            {/* Project Node Header */}
            <button
              type="button"
              onClick={() => setIsChatsTreeExpanded(!isChatsTreeExpanded)}
              className="w-full flex items-center justify-between px-2 py-1 text-zinc-400 hover:text-zinc-200 text-xs font-medium tracking-wide uppercase"
            >
              <div className="flex items-center gap-1.5">
                <span className="text-[10px]">{isChatsTreeExpanded ? '▼' : '▶'}</span>
                <span>Chats &amp; Tasks</span>
              </div>
              <span className="text-[10px] font-mono text-zinc-500">{chats.length}</span>
            </button>

            {/* List of Chats */}
            {isChatsTreeExpanded && (
              <div className="space-y-0.5 pl-2">
                {chats.length === 0 ? (
                  <div className="px-2 py-3 text-[11px] text-zinc-600 italic">
                    No tasks yet. Start a new chat below.
                  </div>
                ) : (
                  chats.map((chat) => {
                    const isSelected = activeChatId === chat.id;
                    return (
                      <div
                        key={chat.id}
                        className={`group w-full px-2.5 py-2 rounded-md transition-colors flex items-center justify-between cursor-pointer ${
                          isSelected
                            ? 'bg-zinc-800/90 text-white font-medium border border-zinc-700/80 shadow-sm'
                            : 'text-zinc-400 hover:bg-zinc-800/40 hover:text-zinc-200'
                        }`}
                        onClick={() => onSelectChat(chat.id)}
                      >
                        <div className="flex-1 truncate mr-1">
                          <div className="flex items-center justify-between gap-1 w-full">
                            <span className="text-xs truncate">{chat.title}</span>
                            {statusBadge(chat.status)}
                          </div>
                          <div className="flex items-center justify-between text-[10px] text-zinc-500 font-mono mt-0.5">
                            <span>{chat.created_at}</span>
                            {chat.files_count !== undefined && chat.files_count > 0 && (
                              <span>{chat.files_count} file(s)</span>
                            )}
                          </div>
                        </div>

                        {onDeleteChat && (
                          <button
                            type="button"
                            onClick={(e) => {
                              e.stopPropagation();
                              onDeleteChat(chat.id);
                            }}
                            title="Delete Chat"
                            className="opacity-0 group-hover:opacity-100 p-1 hover:text-red-400 text-zinc-500 transition-opacity font-mono text-xs"
                          >
                            &times;
                          </button>
                        )}
                      </div>
                    );
                  })
                )}
              </div>
            )}
          </div>
        ) : (
          <div className="p-3 text-center text-xs text-zinc-500 space-y-2">
            <div>No project open</div>
            <button
              type="button"
              onClick={onOpenProjectDialog}
              className="px-2.5 py-1 rounded bg-zinc-800 text-zinc-300 hover:text-white text-xs transition-colors"
            >
              Open Project
            </button>
          </div>
        )}
      </div>

      {/* Bottom Quiet Engine Status & Quota */}
      <div className="p-2.5 border-t border-zinc-800/80 bg-[#09090b] text-[11px] space-y-2 font-sans">
        {/* Planner Quota telemetry quick trigger */}
        <button
          type="button"
          onClick={onOpenQuota}
          className="w-full flex items-center justify-between px-2 py-1 rounded hover:bg-zinc-800/60 transition-colors text-zinc-400 hover:text-zinc-200"
          title="Rate Limits & Quota"
        >
          <span className="text-[10px] font-mono">
            {quotaState?.active_model || 'gemini-3.5-flash-lite'}
          </span>
          <span className="text-[10px] font-mono text-zinc-500">
            {quotaState?.models[0]
              ? `${quotaState.models[0].rpm_remaining}/${quotaState.models[0].rpm_limit} RPM`
              : 'limits'}
          </span>
        </button>

        {/* Quiet Subsystem Dots with Tooltips */}
        <div className="flex items-center justify-around pt-1 border-t border-zinc-900 text-zinc-500">
          <div
            className="flex items-center gap-1 cursor-help"
            title={`Gemini: ${isGeminiReady ? 'Ready (Cloud Planner)' : 'Missing Key'}`}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                isGeminiReady ? 'bg-emerald-400' : 'bg-zinc-600'
              }`}
            />
            <span className="text-[10px]">Cloud</span>
          </div>

          <div
            className="flex items-center gap-1 cursor-help"
            title={`Ollama: ${isOllamaReady ? 'Ready (qwen3.5:9b)' : 'Offline'}`}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                isOllamaReady ? 'bg-emerald-400' : 'bg-amber-500'
              }`}
            />
            <span className="text-[10px]">Worker</span>
          </div>

          <div
            className="flex items-center gap-1 cursor-help"
            title={`OpenCode Sidecar: ${formattedOpencode}`}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                isOpencodeReady ? 'bg-emerald-400' : 'bg-zinc-600'
              }`}
            />
            <span className="text-[10px]">Sidecar</span>
          </div>
        </div>
      </div>
    </aside>
  );
};