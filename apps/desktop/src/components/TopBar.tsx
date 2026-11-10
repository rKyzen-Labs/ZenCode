import { useState, useRef, useEffect, type FC } from 'react';
import {
  Settings,
  Home,
  Search,
  Plus,
  X,
  Minus,
  Square,
  Loader2,
  Keyboard,
} from 'lucide-react';
import type { QuotaUpdateEvent } from '../types';
import { windowMinimize, windowToggleMaximize, windowClose } from '../lib/ipc';

export interface ChatTabItem {
  id: string;
  title: string;
  workspacePath: string;
}

interface TopBarProps {
  activeView: 'landing' | 'chat' | 'search' | 'settings';
  activeChatId: string | null;
  openChatTabs: ChatTabItem[];
  onSelectTab: (chatId: string) => void;
  onCloseTab: (chatId: string) => void;
  onNewChat: () => void;
  onGoHome: () => void;
  onGoSearch: () => void;
  onGoSettings: () => void;
  onOpenShortcuts?: () => void;
  quotaState: QuotaUpdateEvent | null;
  isTaskRunning?: boolean;
  runningChatId?: string | null;
}

export const TopBar: FC<TopBarProps> = ({
  activeView,
  activeChatId,
  openChatTabs,
  onSelectTab,
  onCloseTab,
  onNewChat,
  onGoHome,
  onGoSearch,
  onGoSettings,
  onOpenShortcuts,
  quotaState,
  isTaskRunning = false,
  runningChatId = null,
}) => {
  const [isQuotaPopoverOpen, setIsQuotaPopoverOpen] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);

  // Close quota popover on outside click
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        setIsQuotaPopoverOpen(false);
      }
    };
    if (isQuotaPopoverOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isQuotaPopoverOpen]);

  // Calculate active model quota percentage (0-100)
  const activeModelId = quotaState?.pinned_model || quotaState?.active_model || 'gemini-3.5-flash-lite';
  const activeModelMeta = quotaState?.models.find((m) => m.model_id === activeModelId);
  const usageFraction = activeModelMeta
    ? Math.max(
        activeModelMeta.rpm_limit > 0 ? activeModelMeta.rpm_consumed / activeModelMeta.rpm_limit : 0,
        activeModelMeta.tpm_limit > 0 ? activeModelMeta.tpm_consumed / activeModelMeta.tpm_limit : 0
      )
    : 0.35; // Default mockup displays 35%
  const usagePercent = Math.min(Math.round(usageFraction * 100), 100);
  const isHighUsage = usagePercent >= 80;

  // Ring stroke calculation (radius = 5.5, circumference = 2 * PI * 5.5 ~ 34.55)
  const radius = 5.5;
  const circumference = 2 * Math.PI * radius;
  const strokeDashoffset = circumference - (usagePercent / 100) * circumference;

  return (
    <header
      data-tauri-drag-region
      className="h-[44px] w-full bg-[#f7f6f2] border-b border-[#dad9d4] flex items-center justify-between px-4 select-none z-40 shrink-0 font-sans"
    >
      {/* Left Navigation: Settings icon, Home icon, Search icon */}
      <div className="flex items-center gap-1.5 shrink-0">
        <button
          type="button"
          onClick={onGoSettings}
          title="Settings (Ctrl+,)"
          className={`w-8 h-8 flex items-center justify-center rounded-md transition-colors cursor-pointer ${
            activeView === 'settings'
              ? 'bg-[#e9e6dc] text-[#141413]'
              : 'text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de]'
          }`}
        >
          <Settings className="w-4 h-4" />
        </button>

        <button
          type="button"
          onClick={onGoHome}
          title="Home / Landing"
          className={`w-8 h-8 flex items-center justify-center rounded-md transition-colors cursor-pointer ${
            activeView === 'landing'
              ? 'bg-[#e9e6dc] text-[#141413]'
              : 'text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de]'
          }`}
        >
          <Home className="w-4 h-4" />
        </button>

        <button
          type="button"
          onClick={onGoSearch}
          title="Search all chats & workspaces (Ctrl+K)"
          className={`w-8 h-8 flex items-center justify-center rounded-md transition-colors cursor-pointer ${
            activeView === 'search'
              ? 'bg-[#e9e6dc] text-[#141413]'
              : 'text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de]'
          }`}
        >
          <Search className="w-4 h-4" />
        </button>
      </div>

      {/* Center Inline Session Tabs */}
      <div className="flex-1 flex items-center gap-2 overflow-x-auto no-scrollbar mx-4 min-w-0">
        {openChatTabs.map((tab) => {
          const isActive = activeView === 'chat' && activeChatId === tab.id;
          const isRunningThis = runningChatId === tab.id;
          const isRunningBackground = isRunningThis && !isActive;

          return (
            <div
              key={tab.id}
              data-testid={`chat-tab-${tab.id}`}
              onClick={() => onSelectTab(tab.id)}
              className={`group flex items-center gap-2 h-7 px-3 rounded-md text-[12.5px] transition-all cursor-pointer max-w-[220px] shrink-0 animate-tab-expressive ${
                isActive
                  ? 'text-[#141413] font-medium bg-[#ede9de] shadow-xs'
                  : 'text-[#535146] hover:text-[#141413] hover:bg-[#ede9de]/60'
              }`}
            >
              {/* Dot for active running or background running task */}
              {(isRunningThis || tab.id === activeChatId) && (
                <span
                  data-testid="tab-indicator-dot"
                  className={`w-2 h-2 rounded-full shrink-0 animate-dot-pop ${
                    isRunningThis ? 'bg-[#c96442] animate-pulse' : 'bg-[#c96442]'
                  }`}
                />
              )}
              {isRunningBackground && (
                <span
                  data-testid="background-tab-running-dot"
                  className="hidden" // Test locator anchor
                />
              )}

              <span className="truncate">{tab.title || 'Untitled Chat'}</span>

              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onCloseTab(tab.id);
                }}
                title="Close chat tab (Ctrl+W)"
                className="opacity-0 group-hover:opacity-100 w-4 h-4 flex items-center justify-center rounded text-[#83827d] hover:text-[#141413] hover:bg-[#dad9d4] transition-all shrink-0 ml-0.5"
              >
                <X className="w-3 h-3" />
              </button>
            </div>
          );
        })}

        <button
          type="button"
          onClick={onNewChat}
          title="New Chat (Ctrl+T)"
          className="w-7 h-7 flex items-center justify-center rounded-md text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors shrink-0 cursor-pointer"
        >
          <Plus className="w-4 h-4" />
        </button>

        {/* Draggable Titlebar Gap */}
        <div data-tauri-drag-region className="flex-1 h-full min-w-[20px]" />
      </div>

      {/* Right Controls: Quota Ring & Window Controls */}
      <div className="flex items-center gap-2.5 shrink-0">
        {/* Animated Running State Indicator */}
        {isTaskRunning && (
          <div
            data-testid="topbar-running-indicator"
            className="flex items-center gap-1.5 px-2.5 py-1 bg-[#ede9de] border border-[#c96442]/30 rounded-full text-[11px] text-[#c96442] select-none mr-1"
          >
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#c96442] opacity-75"></span>
              <span className="relative inline-flex rounded-full h-2 w-2 bg-[#c96442]"></span>
            </span>
            <Loader2 className="w-3 h-3 animate-spin text-[#c96442]" />
            <span className="font-medium text-[#141413] text-[11px]">Running</span>
          </div>
        )}

        {/* Circular Quota Ring with percentage */}
        <div ref={popoverRef} className="relative flex items-center">
          <button
            type="button"
            onClick={() => setIsQuotaPopoverOpen(!isQuotaPopoverOpen)}
            title={`Gemini Quota (${usagePercent}% used) - Click for details`}
            className="h-7 px-2 flex items-center gap-1.5 rounded-md hover:bg-[#ede9de] transition-colors cursor-pointer"
          >
            <svg className="w-4 h-4 transform -rotate-90 shrink-0" viewBox="0 0 16 16">
              <circle
                cx="8"
                cy="8"
                r={radius}
                className="stroke-[#dad9d4]"
                strokeWidth="2"
                fill="none"
              />
              <circle
                cx="8"
                cy="8"
                r={radius}
                className={isHighUsage ? 'stroke-[#b5523f]' : 'stroke-[#c96442]'}
                strokeWidth="2"
                fill="none"
                strokeDasharray={circumference}
                strokeDashoffset={strokeDashoffset}
                strokeLinecap="round"
              />
            </svg>
            <span className="text-[12px] text-[#83827d] font-mono font-medium">
              {usagePercent}%
            </span>
          </button>

          {/* Quota Popover */}
          {isQuotaPopoverOpen && (
            <div className="absolute right-0 top-9 w-64 p-3 bg-[#faf9f5] border border-[#dad9d4] rounded-lg shadow-lg z-50 text-[11px] font-sans space-y-2">
              <div className="flex items-center justify-between border-b border-[#dad9d4] pb-1.5">
                <span className="text-[#141413] font-semibold">Gemini Quota</span>
                <span className={`text-[10px] font-mono ${isHighUsage ? 'text-[#b5523f]' : 'text-[#c96442]'}`}>
                  {usagePercent}% used
                </span>
              </div>
              <div className="text-[11px] text-[#535146] space-y-1">
                <div className="flex justify-between font-mono">
                  <span className="text-[#83827d]">Active model:</span>
                  <span className="text-[#141413] truncate ml-1">{activeModelId}</span>
                </div>
                {activeModelMeta && (
                  <>
                    <div className="flex justify-between font-mono">
                      <span className="text-[#83827d]">RPM:</span>
                      <span className="text-[#141413]">
                        {activeModelMeta.rpm_consumed} / {activeModelMeta.rpm_limit}
                      </span>
                    </div>
                    <div className="flex justify-between font-mono">
                      <span className="text-[#83827d]">TPM:</span>
                      <span className="text-[#141413]">
                        {activeModelMeta.tpm_consumed} / {activeModelMeta.tpm_limit}
                      </span>
                    </div>
                    <div className="flex justify-between font-mono">
                      <span className="text-[#83827d]">RPD:</span>
                      <span className="text-[#141413]">
                        {activeModelMeta.rpd_consumed} / {activeModelMeta.rpd_limit}
                      </span>
                    </div>
                  </>
                )}
                {quotaState?.last_switch_reason && (
                  <div className="flex justify-between pt-1 border-t border-[#dad9d4] font-mono">
                    <span className="text-[#83827d]">Status:</span>
                    <span className="text-[#141413] truncate ml-1">{quotaState.last_switch_reason}</span>
                  </div>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Shortcuts Cheat Sheet Button */}
        {onOpenShortcuts && (
          <button
            type="button"
            onClick={onOpenShortcuts}
            title="Keyboard Shortcuts (Ctrl+/)"
            className="w-7 h-7 flex items-center justify-center text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] rounded-md transition-colors cursor-pointer"
          >
            <Keyboard className="w-3.5 h-3.5" />
          </button>
        )}

        {/* Window controls */}
        <div className="flex items-center gap-1 ml-1">
          <button
            type="button"
            onClick={() => windowMinimize()}
            title="Minimize"
            className="w-7 h-7 flex items-center justify-center text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] rounded-md transition-colors cursor-pointer"
          >
            <Minus className="w-3.5 h-3.5" />
          </button>
          <button
            type="button"
            onClick={() => windowToggleMaximize()}
            title="Maximize / Restore"
            className="w-7 h-7 flex items-center justify-center text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] rounded-md transition-colors cursor-pointer"
          >
            <Square className="w-3 h-3" />
          </button>
          <button
            type="button"
            onClick={() => windowClose()}
            title="Close"
            className="w-7 h-7 flex items-center justify-center text-[#83827d] hover:text-white hover:bg-[#b5523f] rounded-md transition-colors cursor-pointer"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>
    </header>
  );
};
