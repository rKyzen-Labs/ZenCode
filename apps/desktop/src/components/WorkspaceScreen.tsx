import { useState, type FC, type FormEvent } from 'react';
import type { ProjectState } from '../types';
import {
  FolderGit2,
  Brain,
  Cpu,
  ArrowRight,
  Shield,
  FileCode,
  Sparkles,
  RotateCcw,
} from 'lucide-react';

interface WorkspaceScreenProps {
  project: ProjectState;
  onPreviewContext: (taskDescription: string) => Promise<void>;
  onChangeProject: () => void;
  isLoading: boolean;
  error: string | null;
}

export const WorkspaceScreen: FC<WorkspaceScreenProps> = ({
  project,
  onPreviewContext,
  onChangeProject,
  isLoading,
  error,
}) => {
  const [taskDescription, setTaskDescription] = useState('');

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (taskDescription.trim()) {
      onPreviewContext(taskDescription.trim());
    }
  };

  return (
    <div className="flex-1 flex flex-col p-6 max-w-5xl mx-auto w-full space-y-6">
      {/* Active Project Banner */}
      <div className="bg-[var(--surface)] border border-[var(--surface-border)] rounded-xl p-5 shadow-lg flex items-center justify-between">
        <div className="flex items-center gap-4">
          <div className="w-11 h-11 rounded-lg bg-blue-500/10 border border-blue-500/30 flex items-center justify-center text-blue-400">
            <FolderGit2 className="w-6 h-6" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <h1 className="text-base font-semibold text-zinc-100">{project.name}</h1>
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-[var(--surface-raised)] border border-[var(--surface-border)] text-zinc-300">
                {project.project_type}
              </span>
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-blue-500/10 border border-blue-500/30 text-blue-400">
                {project.current_branch || 'no-branch'}
              </span>
            </div>
            <p className="text-xs text-[var(--foreground-muted)] font-mono mt-0.5">
              {project.path}
            </p>
          </div>
        </div>

        <button
          type="button"
          onClick={onChangeProject}
          className="px-3 py-1.5 rounded-lg bg-[var(--surface-raised)] hover:bg-zinc-800 border border-[var(--surface-border)] text-xs text-zinc-300 transition-colors flex items-center gap-1.5"
        >
          <RotateCcw className="w-3.5 h-3.5 text-zinc-400" />
          <span>Switch Project</span>
        </button>
      </div>

      {/* Model Configuration Strip */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        <div className="p-3.5 rounded-lg bg-[var(--surface)] border border-[var(--surface-border)] flex items-center gap-3">
          <div className="w-8 h-8 rounded-md bg-purple-500/10 border border-purple-500/30 flex items-center justify-center text-purple-400 shrink-0">
            <Brain className="w-4 h-4" />
          </div>
          <div>
            <div className="flex items-center gap-1.5">
              <span className="text-xs font-semibold text-zinc-200">Planner: Gemini 3.5 Flash-Lite</span>
              <span className="text-[9px] px-1.5 py-0.2 rounded bg-purple-500/10 text-purple-400 border border-purple-500/20">Free Tier</span>
            </div>
            <p className="text-[11px] text-[var(--foreground-muted)]">
              Explicit single plan call (store=false). Zero code editing capability.
            </p>
          </div>
        </div>

        <div className="p-3.5 rounded-lg bg-[var(--surface)] border border-[var(--surface-border)] flex items-center gap-3">
          <div className="w-8 h-8 rounded-md bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 shrink-0">
            <Cpu className="w-4 h-4" />
          </div>
          <div>
            <div className="flex items-center gap-1.5">
              <span className="text-xs font-semibold text-zinc-200">Worker: Ollama qwen2.5-coder:7b</span>
              <span className="text-[9px] px-1.5 py-0.2 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">Local Inference</span>
            </div>
            <p className="text-[11px] text-[var(--foreground-muted)]">
              All editing, building, tool calls, and repair loops remain on your Windows 11 machine.
            </p>
          </div>
        </div>
      </div>

      {/* Task Input Section */}
      <div className="bg-[var(--surface)] border border-[var(--surface-border)] rounded-xl p-6 shadow-xl space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2 text-sm font-semibold text-zinc-200">
            <Sparkles className="w-4 h-4 text-blue-400" />
            <span>Describe Your Task</span>
          </div>
          <span className="text-[11px] text-[var(--foreground-subtle)] flex items-center gap-1">
            <Shield className="w-3.5 h-3.5 text-emerald-400" />
            Payload inspected locally before transmission
          </span>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="relative">
            <textarea
              rows={5}
              value={taskDescription}
              onChange={(e) => setTaskDescription(e.target.value)}
              placeholder="E.g., Implement an async health-check endpoint that pings dependencies, or refactor state persistence into SQLite..."
              className="w-full bg-[var(--surface-raised)] border border-[var(--surface-border)] rounded-lg p-3.5 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-blue-500 font-sans leading-relaxed resize-none"
              disabled={isLoading}
            />
          </div>

          {error && (
            <div className="p-3 rounded-lg bg-rose-500/10 border border-rose-500/30 text-rose-400 text-xs">
              {error}
            </div>
          )}

          <div className="flex items-center justify-between pt-2">
            <div className="text-[11px] text-[var(--foreground-muted)] flex items-center gap-1.5">
              <FileCode className="w-3.5 h-3.5 text-zinc-400" />
              <span>Next step: review full context package before sending to Gemini</span>
            </div>

            <button
              type="submit"
              disabled={isLoading || !taskDescription.trim()}
              className="px-5 py-2.5 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:opacity-50 text-white font-medium text-sm flex items-center gap-2 transition-colors shadow-sm"
            >
              <span>{isLoading ? 'Assembling Context...' : 'Preview Planning Context'}</span>
              <ArrowRight className="w-4 h-4" />
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
