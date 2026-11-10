import React from 'react';
import { HelpCircle, AlertTriangle, Sparkles, X, Loader2 } from 'lucide-react';
import type { AdvisoryPreview } from '../types';

interface AdvisoryPreviewModalProps {
  preview: AdvisoryPreview | null;
  isOpen: boolean;
  isLoading: boolean;
  onConfirm: (previewId: string) => void;
  onCancel?: () => void;
  onClose?: () => void;
  isQuotaExhausted?: boolean;
  exhaustionReason?: string;
}

export const AdvisoryPreviewModal: React.FC<AdvisoryPreviewModalProps> = ({
  preview,
  isOpen,
  isLoading,
  onConfirm,
  onCancel,
  onClose,
  isQuotaExhausted = false,
  exhaustionReason,
}) => {
  if (!isOpen || !preview) return null;
  const handleCancel = onCancel || onClose || (() => {});

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm p-4 font-mono text-xs select-none">
      <div className="w-full max-w-xl border border-[#262626] bg-[#0c0c0c] p-6 shadow-2xl space-y-4 rounded-lg">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-[#262626] pb-3">
          <div className="flex items-center gap-2">
            <HelpCircle className="w-4 h-4 text-[#38bdf8]" />
            <span className="text-[#fafafa] font-semibold tracking-wider uppercase text-xs">
              Advisory Consultation Context Preview
            </span>
          </div>
          <button
            type="button"
            onClick={handleCancel}
            className="text-zinc-500 hover:text-zinc-300 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Advisory Boundaries Notice */}
        <div className="text-zinc-400 text-[11px] leading-relaxed">
          The local worker has paused after repeated repair attempts. You may request an advisory suggestion from Gemini.
          Only the minimal error signature and relevant source file excerpt will be sent:
        </div>

        {/* Payload Preview */}
        <div className="space-y-2">
          <div className="text-[10px] text-zinc-500 uppercase font-semibold">
            Error Signature
          </div>
          <pre className="p-2.5 bg-[#141414] border border-[#262626] text-rose-400 text-[11px] whitespace-pre-wrap max-h-32 overflow-y-auto rounded">
            {preview.error_text}
          </pre>
        </div>

        {preview.file_path && (
          <div className="space-y-1">
            <div className="text-[10px] text-zinc-500 uppercase font-semibold">
              Relevant File: <span className="text-zinc-300 font-normal">{preview.file_path}</span>
            </div>
            <pre className="p-2 bg-[#141414] border border-[#262626] text-zinc-300 text-[10px] whitespace-pre-wrap max-h-32 overflow-y-auto rounded font-mono">
              {preview.file_excerpt}
            </pre>
          </div>
        )}

        {/* Guarantees Box */}
        <div className="text-[10px] text-zinc-500 border border-[#262626] bg-[#101010] p-2.5 space-y-1 rounded">
          <div className="flex items-center gap-1.5 text-zinc-400 font-medium">
            <Sparkles className="w-3 h-3 text-[#38bdf8]" />
            <span>Advisory Guarantees</span>
          </div>
          <div>• Gemini provides advice only. It has zero filesystem or execution access.</div>
          <div>• Local worker tests and validates all implementation changes locally.</div>
          <div>• Consumes free-tier quota (monitored by rate-limiter cascade).</div>
        </div>

        {/* Quota Exhaustion Warning */}
        {isQuotaExhausted && (
          <div className="p-2.5 border border-rose-900/50 bg-rose-950/20 text-rose-400 text-[11px] rounded flex items-center gap-2">
            <AlertTriangle className="w-4 h-4 shrink-0 text-rose-400" />
            <span>
              Cannot submit: Gemini quota is currently exhausted ({exhaustionReason || 'rate limit reached'}). Please wait for the quota window to reset.
            </span>
          </div>
        )}

        {/* Actions */}
        <div className="flex items-center justify-end gap-3 pt-2 border-t border-[#262626]">
          <button
            type="button"
            onClick={handleCancel}
            disabled={isLoading}
            className="px-3 py-1.5 border border-[#262626] bg-[#141414] hover:bg-[#1f1f1f] text-zinc-400 hover:text-zinc-200 transition-colors disabled:opacity-40 rounded"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => onConfirm(preview.preview_id)}
            disabled={isLoading || isQuotaExhausted}
            className="px-3.5 py-1.5 border border-[#38bdf8] bg-[#38bdf8] hover:bg-[#0284c7] text-[#0a0a0a] font-semibold transition-colors disabled:opacity-40 rounded flex items-center gap-2"
          >
            {isLoading ? (
              <>
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                <span>Consulting Gemini...</span>
              </>
            ) : isQuotaExhausted ? (
              <span>Quota Exhausted</span>
            ) : (
              <span>Confirm & Request Advice</span>
            )}
          </button>
        </div>
      </div>
    </div>
  );
};
