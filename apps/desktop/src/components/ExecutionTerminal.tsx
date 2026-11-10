import React, { useEffect, useRef } from 'react';
import { Terminal, X, Square, Download, CheckCircle2, AlertCircle, Clock } from 'lucide-react';
import type { ExecutionResult, ProcessOutputEvent } from '../types';

interface ExecutionTerminalProps {
  isOpen: boolean;
  commandId: string | null;
  program: string;
  args: string[];
  cwd: string;
  isRunning: boolean;
  outputLines: ProcessOutputEvent[];
  result: ExecutionResult | null;
  onCancel: () => void;
  onClose: () => void;
  onExportDiagnostics?: () => void;
}

export const ExecutionTerminal: React.FC<ExecutionTerminalProps> = ({
  isOpen,
  commandId,
  program,
  args,
  cwd,
  isRunning,
  outputLines,
  result,
  onCancel,
  onClose,
  onExportDiagnostics,
}) => {
  const terminalEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (isOpen) {
      terminalEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [outputLines, isOpen]);

  if (!isOpen) return null;

  const fullCommand = `${program} ${args.join(' ')}`.trim();

  // Status Badge
  const renderStatus = () => {
    if (isRunning) {
      return (
        <span className="text-[#38bdf8] font-semibold animate-pulse flex items-center gap-1">
          <span className="w-1.5 h-1.5 rounded-full bg-[#38bdf8]" />
          <span>Running...</span>
        </span>
      );
    }
    if (!result) return null;
    if (result.cancelled) {
      return (
        <span className="text-zinc-500 font-semibold flex items-center gap-1">
          <AlertCircle className="w-3 h-3 text-zinc-500" />
          <span>Cancelled</span>
        </span>
      );
    }
    if (result.success || result.exit_code === 0) {
      return (
        <span className="text-emerald-400 font-semibold flex items-center gap-1">
          <CheckCircle2 className="w-3 h-3 text-emerald-400" />
          <span>Exit: 0</span>
        </span>
      );
    }
    return (
      <span className="text-rose-400 font-semibold flex items-center gap-1">
        <AlertCircle className="w-3 h-3 text-rose-400" />
        <span>Exit: {result.exit_code ?? 'Error'}</span>
      </span>
    );
  };

  return (
    <div className="border border-[#262626] bg-[#0c0c0c] rounded-lg font-mono text-xs overflow-hidden shadow-2xl mt-4">
      {/* Title bar */}
      <div className="flex items-center justify-between border-b border-[#262626] bg-[#141414] px-4 py-2">
        <div className="flex items-center gap-3 overflow-hidden">
          <Terminal className="w-3.5 h-3.5 text-[#38bdf8] shrink-0" />
          <span className="text-zinc-200 font-medium truncate max-w-md">
            $ {fullCommand}
          </span>
          {renderStatus()}
          {result && (
            <span className="text-zinc-500 text-[10px] flex items-center gap-1">
              <Clock className="w-2.5 h-2.5" />
              <span>{result.duration_ms}ms</span>
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {isRunning && (
            <button
              onClick={onCancel}
              className="px-2 py-0.5 border border-rose-900/60 bg-rose-950/30 hover:bg-rose-950/50 text-rose-300 text-[11px] rounded transition-colors flex items-center gap-1"
            >
              <Square className="w-3 h-3 fill-current" />
              <span>Stop</span>
            </button>
          )}
          {onExportDiagnostics && !isRunning && (
            <button
              onClick={onExportDiagnostics}
              className="px-2 py-0.5 border border-[#262626] bg-[#1a1a1a] hover:bg-[#262626] text-zinc-300 text-[11px] rounded transition-colors flex items-center gap-1"
            >
              <Download className="w-3 h-3" />
              <span>Export Diagnostics</span>
            </button>
          )}
          <button
            onClick={onClose}
            className="text-zinc-500 hover:text-zinc-200 transition-colors p-0.5"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Directory context */}
      <div className="border-b border-[#1c1c1c] bg-[#101010] px-4 py-1 text-[10px] text-zinc-500 flex items-center gap-2">
        <span>cwd:</span>
        <span className="text-zinc-400 truncate">{cwd}</span>
        {commandId && <span className="ml-auto text-zinc-600">id: {commandId}</span>}
      </div>

      {/* Terminal Output stream */}
      <div className="h-64 overflow-y-auto p-3 bg-[#0a0a0a] space-y-0.5 font-mono text-[11px] leading-5 selection:bg-zinc-800">
        {outputLines.length === 0 ? (
          <div className="text-zinc-600 italic">Awaiting process stream...</div>
        ) : (
          outputLines.map((ev, idx) => (
            <div key={idx} className="flex items-start gap-2">
              <span className="text-zinc-700 select-none w-8 text-right shrink-0">
                {idx + 1}
              </span>
              <span
                className={`w-14 shrink-0 text-[10px] select-none font-semibold ${
                  ev.stream === 'stderr' ? 'text-rose-400' : 'text-zinc-500'
                }`}
              >
                {ev.stream}
              </span>
              <pre
                className={`flex-1 whitespace-pre-wrap break-all ${
                  ev.stream === 'stderr' ? 'text-zinc-100 font-medium' : 'text-zinc-300'
                }`}
              >
                {ev.line}
              </pre>
            </div>
          ))
        )}
        <div ref={terminalEndRef} />
      </div>

      {/* Footer info */}
      {result && result.error_message && (
        <div className="border-t border-rose-900/50 bg-rose-950/20 px-4 py-1.5 text-[11px] text-rose-400 flex items-center gap-1.5">
          <AlertCircle className="w-3.5 h-3.5 shrink-0" />
          <span>Error: {result.error_message}</span>
        </div>
      )}
    </div>
  );
};
