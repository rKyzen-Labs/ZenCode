import React from 'react';
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
      <div className="w-full max-w-xl border border-zinc-700 bg-zinc-950 p-6 shadow-2xl space-y-4">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-zinc-800 pb-3">
          <div className="flex items-center gap-2">
            <span className="text-zinc-300 font-bold">[?]</span>
            <span className="text-zinc-100 font-bold tracking-wider uppercase">
              Advisory Consultation Context Preview
            </span>
          </div>
          <span className="text-[10px] text-zinc-500">{preview.preview_id}</span>
        </div>

        {/* Advisory Boundaries Notice */}
        <div className="text-zinc-400 text-[11px] leading-relaxed">
          The local worker has paused after repeated repair attempts. You may request an advisory suggestion from Gemini.
          Only the minimal error signature and relevant source file excerpt will be sent:
        </div>

        {/* Payload Preview */}
        <div className="space-y-2">
          <div className="text-[10px] text-zinc-500 uppercase font-semibold">
            Error Signature:
          </div>
          <pre className="p-2.5 bg-zinc-900/80 border border-zinc-800 text-danger text-[11px] whitespace-pre-wrap max-h-32 overflow-y-auto rounded">
            {preview.error_text}
          </pre>
        </div>

        {preview.file_path && (
          <div className="space-y-1">
            <div className="text-[10px] text-zinc-500 uppercase font-semibold">
              Relevant File: <span className="text-zinc-300 font-normal">{preview.file_path}</span>
            </div>
            <pre className="p-2 bg-zinc-900/60 border border-zinc-800 text-zinc-300 text-[10px] whitespace-pre-wrap max-h-32 overflow-y-auto rounded font-mono">
              {preview.file_excerpt}
            </pre>
          </div>
        )}

        {/* Guarantees Box */}
        <div className="text-[10px] text-zinc-500 border border-zinc-900 bg-zinc-950 p-2.5 space-y-1">
          <div>* Gemini provides advice only. It has zero filesystem or execution access.</div>
          <div>* Local worker tests and validates all implementation changes locally.</div>
          <div>* Consumes free-tier quota (monitored by rate-limiter cascade).</div>
        </div>

        {/* Quota Exhaustion Warning */}
        {isQuotaExhausted && (
          <div className="p-2.5 border border-danger/40 bg-danger/10 text-danger text-[11px] rounded">
            Cannot submit: Gemini quota is currently exhausted ({exhaustionReason || 'rate limit reached'}). Please wait for the quota window to reset.
          </div>
        )}

        {/* Actions */}
        <div className="flex items-center justify-end gap-3 pt-2 border-t border-zinc-800">
          <button
            type="button"
            onClick={handleCancel}
            disabled={isLoading}
            className="px-3 py-1.5 border border-zinc-700 bg-zinc-900 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 transition-colors disabled:opacity-40"
          >
            [cancel]
          </button>
          <button
            type="button"
            onClick={() => onConfirm(preview.preview_id)}
            disabled={isLoading || isQuotaExhausted}
            className="px-3 py-1.5 border border-zinc-200 bg-zinc-100 hover:bg-white text-zinc-950 font-bold transition-colors disabled:opacity-40"
          >
            {isLoading ? '[consulting_gemini...]' : isQuotaExhausted ? '[quota_exhausted]' : '[confirm_and_request_advice]'}
          </button>
        </div>
      </div>
    </div>
  );
};
