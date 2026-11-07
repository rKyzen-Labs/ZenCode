import React, { useEffect, useRef } from 'react';
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
        <span className="text-amber-400 font-bold animate-pulse">
          [running...]
        </span>
      );
    }
    if (!result) return null;
    if (result.cancelled) {
      return <span className="text-amber-500 font-bold">[cancelled]</span>;
    }
    if (result.success || result.exit_code === 0) {
      return <span className="text-emerald-400 font-bold">[exit: 0 ok]</span>;
    }
    return (
      <span className="text-red-400 font-bold">
        [exit: {result.exit_code ?? 'err'} failed]
      </span>
    );
  };

  return (
    <div className="border border-zinc-800 bg-zinc-950 rounded font-mono text-xs overflow-hidden shadow-2xl mt-4">
      {/* Title bar */}
      <div className="flex items-center justify-between border-b border-zinc-800 bg-zinc-900/80 px-4 py-2">
        <div className="flex items-center gap-3 overflow-hidden">
          <span className="text-zinc-500 font-bold">[exec]</span>
          <span className="text-zinc-200 font-semibold truncate max-w-md">
            $ {fullCommand}
          </span>
          {renderStatus()}
          {result && (
            <span className="text-zinc-500 text-[10px]">
              ({result.duration_ms}ms)
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {isRunning && (
            <button
              onClick={onCancel}
              className="px-2 py-0.5 border border-red-500/40 bg-red-950/30 hover:bg-red-900/50 text-red-300 text-[11px] transition-colors"
            >
              [cancel_execution]
            </button>
          )}
          {onExportDiagnostics && !isRunning && (
            <button
              onClick={onExportDiagnostics}
              className="px-2 py-0.5 border border-zinc-700 bg-zinc-800 hover:bg-zinc-700 text-zinc-300 text-[11px] transition-colors"
            >
              [export_diagnostics]
            </button>
          )}
          <button
            onClick={onClose}
            className="px-2 py-0.5 border border-zinc-800 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 text-[11px] transition-colors"
          >
            [close]
          </button>
        </div>
      </div>

      {/* Directory context */}
      <div className="border-b border-zinc-900 bg-zinc-950/70 px-4 py-1 text-[10px] text-zinc-500 flex items-center gap-2">
        <span>cwd:</span>
        <span className="text-zinc-400 truncate font-sans">{cwd}</span>
        {commandId && <span className="ml-auto text-zinc-600">id: {commandId}</span>}
      </div>

      {/* Terminal Output stream */}
      <div className="h-64 overflow-y-auto p-3 bg-zinc-950 space-y-0.5 font-mono text-[11px] leading-5 selection:bg-zinc-700">
        {outputLines.length === 0 ? (
          <div className="text-zinc-600 italic">Awaiting process stream...</div>
        ) : (
          outputLines.map((ev, idx) => (
            <div key={idx} className="flex items-start gap-2">
              <span className="text-zinc-700 select-none w-8 text-right shrink-0">
                {idx + 1}
              </span>
              <span
                className={`w-14 shrink-0 text-[10px] select-none ${
                  ev.stream === 'stderr' ? 'text-amber-500/70' : 'text-zinc-600'
                }`}
              >
                [{ev.stream}]
              </span>
              <pre
                className={`flex-1 whitespace-pre-wrap break-all ${
                  ev.stream === 'stderr' ? 'text-amber-300/90' : 'text-zinc-300'
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
        <div className="border-t border-red-900/50 bg-red-950/20 px-4 py-1.5 text-[11px] text-red-300">
          [error]: {result.error_message}
        </div>
      )}
    </div>
  );
};
