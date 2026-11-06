import { type FC } from 'react';
import type { QuotaUpdateEvent } from '../types';

interface QuotaDashboardProps {
  quotaState: QuotaUpdateEvent | null;
  onPinModel: (model: string | null) => Promise<void>;
  onClose?: () => void;
  isPinnedActionLoading?: boolean;
}

export const QuotaDashboard: FC<QuotaDashboardProps> = ({
  quotaState,
  onPinModel,
  onClose,
  isPinnedActionLoading,
}) => {
  if (!quotaState) {
    return (
      <div className="p-4 bg-[#121214] border border-[#27272a] rounded text-xs font-mono text-zinc-500">
        [loading_telemetry...]
      </div>
    );
  }

  const isGemini38Pinned = quotaState.pinned_model === 'gemini-3.8-flash';

  const renderMeter = (consumed: number, limit: number) => {
    const ratio = limit > 0 ? Math.min(consumed / limit, 1) : 0;
    const totalBars = 16;
    const filledBars = Math.round(ratio * totalBars);
    const emptyBars = totalBars - filledBars;
    return `[${'#'.repeat(filledBars)}${'-'.repeat(emptyBars)}] ${(ratio * 100).toFixed(0)}%`;
  };

  return (
    <div className="bg-[#121214] border border-[#27272a] rounded p-4 space-y-4 max-w-4xl mx-auto w-full font-mono text-xs select-none">
      {/* Top Header */}
      <div className="flex items-center justify-between border-b border-[#27272a] pb-3">
        <div className="space-y-0.5">
          <div className="flex items-center gap-2">
            <span className="font-semibold text-zinc-100">[rate_limit_telemetry]</span>
            <span className="text-[10px] text-zinc-400 border border-[#27272a] px-1.5 rounded">
              rust quota manager
            </span>
          </div>
          <p className="text-[11px] text-zinc-500">
            Sliding window RPM / TPM &amp; daily RPD tracking with 80% threshold auto-cascade.
          </p>
        </div>

        <div className="flex items-center gap-2">
          {/* Pin/Unpin Gemini 3.8 Flash */}
          <button
            type="button"
            onClick={() => onPinModel(isGemini38Pinned ? null : 'gemini-3.8-flash')}
            disabled={isPinnedActionLoading}
            className={`px-2.5 py-1 rounded text-xs transition-colors border ${
              isGemini38Pinned
                ? 'bg-zinc-200 text-black border-white font-medium'
                : 'bg-[#18181b] hover:bg-[#27272a] text-zinc-300 border-[#27272a]'
            }`}
          >
            {isGemini38Pinned ? '[* pinned: 3.8-flash]' : '[pin: gemini-3.8-flash]'}
          </button>

          {onClose && (
            <button
              type="button"
              onClick={onClose}
              className="text-zinc-500 hover:text-zinc-200 px-1"
            >
              [x]
            </button>
          )}
        </div>
      </div>

      {/* Switch Reason / Active Banner */}
      <div className="p-2.5 rounded bg-[#09090b] border border-[#27272a] flex items-center justify-between text-xs">
        <div className="flex items-center gap-2">
          <span className="text-zinc-500">active_routing:</span>
          <span className="text-zinc-200 font-semibold">{quotaState.active_model}</span>
          {isGemini38Pinned && (
            <span className="text-zinc-400 border border-[#3f3f46] px-1 rounded text-[10px]">
              manual_pin
            </span>
          )}
        </div>
        {quotaState.last_switch_reason && (
          <div className="text-[10px] text-zinc-500 truncate max-w-sm">
            {quotaState.last_switch_reason}
          </div>
        )}
      </div>

      {/* 4 Models Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        {quotaState.models.map((model) => {
          const isActive = quotaState.active_model === model.model_id;
          const isPinnedThis = quotaState.pinned_model === model.model_id;

          return (
            <div
              key={model.model_id}
              className={`p-3 rounded border space-y-2.5 transition-colors ${
                isActive
                  ? 'bg-[#18181b] border-zinc-500'
                  : 'bg-[#09090b] border-[#27272a]'
              }`}
            >
              {/* Model Header */}
              <div className="flex items-center justify-between">
                <span className="font-semibold text-zinc-200">
                  {model.display_name}
                </span>

                <div className="flex items-center gap-1.5 text-[10px]">
                  {isPinnedThis && (
                    <span className="text-zinc-300 border border-zinc-600 px-1 rounded">
                      pinned
                    </span>
                  )}
                  {isActive && (
                    <span className="text-zinc-100 font-bold">
                      [active]
                    </span>
                  )}
                  {model.threshold_80_percent_exceeded && (
                    <span className="text-zinc-400 border border-[#3f3f46] px-1 rounded">
                      &gt;=80%
                    </span>
                  )}
                </div>
              </div>

              {/* Metrics */}
              <div className="space-y-1.5 text-[11px] text-zinc-400">
                <div className="space-y-0.5">
                  <div className="flex justify-between text-[10px]">
                    <span>RPM: {model.rpm_consumed} / {model.rpm_limit}</span>
                    <span className="text-zinc-500">{renderMeter(model.rpm_consumed, model.rpm_limit)}</span>
                  </div>
                </div>

                <div className="space-y-0.5">
                  <div className="flex justify-between text-[10px]">
                    <span>TPM: {model.tpm_consumed.toLocaleString()} / {model.tpm_limit.toLocaleString()}</span>
                    <span className="text-zinc-500">{renderMeter(model.tpm_consumed, model.tpm_limit)}</span>
                  </div>
                </div>

                <div className="space-y-0.5">
                  <div className="flex justify-between text-[10px]">
                    <span>RPD: {model.rpd_consumed} / {model.rpd_limit}</span>
                    <span className="text-zinc-500">{renderMeter(model.rpd_consumed, model.rpd_limit)}</span>
                  </div>
                </div>
              </div>

              {/* Reset Times */}
              <div className="text-[10px] text-zinc-500 border-t border-[#1f1f23] pt-1 flex justify-between">
                <span>reset in {model.rpm_reset_seconds}s</span>
                <span>daily reset: {model.rpd_reset_formatted}</span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
