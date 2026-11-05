import { useState, type FC, type FormEvent } from 'react';
import type { SystemStatus } from '../types';
import { Folder, KeyRound, Cpu, ShieldCheck, ArrowRight, RefreshCw, AlertCircle, CheckCircle2 } from 'lucide-react';

interface WelcomeScreenProps {
  status: SystemStatus | null;
  onOpenProject: (path: string) => Promise<void>;
  onRefreshStatus: () => Promise<void>;
  isLoading: boolean;
  error: string | null;
}

export const WelcomeScreen: FC<WelcomeScreenProps> = ({
  status,
  onOpenProject,
  onRefreshStatus,
  isLoading,
  error,
}) => {
  const [folderPath, setFolderPath] = useState('A:\\ZenCode');

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (folderPath.trim()) {
      onOpenProject(folderPath.trim());
    }
  };

  const isGeminiReady = status?.gemini_ready ?? false;
  const isOllamaReady = status?.ollama_ready ?? false;

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-8 bg-[var(--background)] max-w-4xl mx-auto w-full">
      <div className="w-full space-y-8">
        {/* Header */}
        <div className="text-center space-y-2">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full border border-blue-500/30 bg-blue-500/10 text-blue-400 text-xs font-medium">
            <span className="w-2 h-2 rounded-full bg-blue-400 animate-pulse"></span>
            Free-First, Local-First AI Engineering
          </div>
          <h1 className="text-3xl font-bold tracking-tight text-zinc-100">Welcome to Zencode</h1>
          <p className="text-sm text-[var(--foreground-muted)] max-w-lg mx-auto">
            Zero cloud spend in the normal workflow. Cloud planning via Gemini 3.5 Flash-Lite, local execution with Ollama.
          </p>
        </div>

        {/* Project Picker Card */}
        <div className="bg-[var(--surface)] border border-[var(--surface-border)] rounded-xl p-6 shadow-xl space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold text-zinc-200 flex items-center gap-2">
              <Folder className="w-4 h-4 text-blue-400" />
              Open Local Project Folder
            </h2>
            <span className="text-xs text-[var(--foreground-subtle)]">Windows 11 Local-First</span>
          </div>

          <form onSubmit={handleSubmit} className="flex gap-2">
            <input
              type="text"
              value={folderPath}
              onChange={(e) => setFolderPath(e.target.value)}
              placeholder="e.g. A:\ZenCode or C:\Users\dev\projects\my-app"
              className="flex-1 bg-[var(--surface-raised)] border border-[var(--surface-border)] rounded-lg px-3.5 py-2.5 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-blue-500 font-mono"
              disabled={isLoading}
            />
            <button
              type="submit"
              disabled={isLoading || !folderPath.trim()}
              className="px-5 py-2.5 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:opacity-50 text-white font-medium text-sm flex items-center gap-2 transition-colors shadow-sm"
            >
              <span>{isLoading ? 'Opening...' : 'Open Project'}</span>
              <ArrowRight className="w-4 h-4" />
            </button>
          </form>

          {/* Preset Buttons */}
          <div className="flex items-center gap-2 pt-1 text-xs text-[var(--foreground-muted)]">
            <span>Quick open:</span>
            <button
              type="button"
              onClick={() => {
                setFolderPath('A:\\ZenCode');
                onOpenProject('A:\\ZenCode');
              }}
              className="px-2.5 py-1 rounded bg-[var(--surface-raised)] hover:bg-zinc-800 border border-[var(--surface-border)] text-zinc-300 font-mono text-[11px] transition-colors"
            >
              A:\ZenCode
            </button>
          </div>

          {error && (
            <div className="p-3 rounded-lg bg-rose-500/10 border border-rose-500/30 text-rose-400 text-xs flex items-center gap-2">
              <AlertCircle className="w-4 h-4 shrink-0" />
              <span>{error}</span>
            </div>
          )}
        </div>

        {/* Engine Readiness Indicators */}
        <div className="bg-[var(--surface)] border border-[var(--surface-border)] rounded-xl p-6 shadow-xl space-y-4">
          <div className="flex items-center justify-between border-b border-[var(--surface-border)] pb-3">
            <div>
              <h3 className="text-sm font-semibold text-zinc-200">Engine Readiness Indicators</h3>
              <p className="text-xs text-[var(--foreground-muted)]">Verification of planning, worker, and sidecar boundaries</p>
            </div>
            <button
              type="button"
              onClick={onRefreshStatus}
              className="p-1.5 rounded-md hover:bg-[var(--surface-raised)] text-[var(--foreground-muted)] hover:text-zinc-200 transition-colors"
              title="Refresh Engine Health"
            >
              <RefreshCw className="w-4 h-4" />
            </button>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
            {/* Gemini Planner Card */}
            <div className="p-3.5 rounded-lg bg-[var(--surface-raised)] border border-[var(--surface-border)] space-y-2">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <KeyRound className="w-4 h-4 text-purple-400" />
                  <span className="text-xs font-semibold text-zinc-200">Gemini Planner</span>
                </div>
                {isGeminiReady ? (
                  <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                ) : (
                  <AlertCircle className="w-4 h-4 text-rose-400" />
                )}
              </div>
              <div className="text-[11px] text-[var(--foreground-muted)]">
                {status?.gemini_status_text || 'Checking OS credentials...'}
              </div>
              <div className="text-[10px] text-zinc-500 font-mono">
                Model: gemini-3.5-flash-lite (store=false)
              </div>
            </div>

            {/* Ollama Worker Card */}
            <div className="p-3.5 rounded-lg bg-[var(--surface-raised)] border border-[var(--surface-border)] space-y-2">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Cpu className="w-4 h-4 text-emerald-400" />
                  <span className="text-xs font-semibold text-zinc-200">Ollama Worker</span>
                </div>
                {isOllamaReady ? (
                  <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                ) : (
                  <AlertCircle className="w-4 h-4 text-rose-400" />
                )}
              </div>
              <div className="text-[11px] text-[var(--foreground-muted)]">
                {status?.ollama_status_text || 'Checking local Ollama service...'}
              </div>
              <div className="text-[10px] text-zinc-500 font-mono">
                Model: qwen2.5-coder:7b (Local inference)
              </div>
            </div>

            {/* OpenCode Sidecar Card */}
            <div className="p-3.5 rounded-lg bg-[var(--surface-raised)] border border-[var(--surface-border)] space-y-2">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <ShieldCheck className="w-4 h-4 text-blue-400" />
                  <span className="text-xs font-semibold text-zinc-200">OpenCode Sidecar</span>
                </div>
                <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/30">
                  {status?.opencode_health || 'Stopped'}
                </span>
              </div>
              <div className="text-[11px] text-[var(--foreground-muted)]">
                Pinned standalone v1.19.5 (loopback 127.0.0.1)
              </div>
              <div className="text-[10px] text-zinc-500 font-mono">
                Auth: Per-process token
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
