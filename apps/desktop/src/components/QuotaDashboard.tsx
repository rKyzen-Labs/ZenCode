import { type FC } from 'react';
import type { QuotaUpdateEvent } from '../types';
import {
  Gauge,
  Clock,
  Pin,
  Sparkles,
  Zap,
  Activity,
  Layers,
  X,
  AlertTriangle,
} from 'lucide-react';

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
      <div className="p-6 bg-[var(--surface)] border border-[var(--surface-border)] rounded-xl text-xs text-zinc-400">
        Loading rate limit telemetry...
      </div>
    );
  }

  const isGemini38Pinned = quotaState.pinned_model === 'gemini-3.8-flash';

  const getMetricBarBg = (consumed: number, limit: number) => {
    const ratio = limit > 0 ? consumed / limit : 0;
    if (ratio >= 0.8) return 'bg-rose-500';
    if (ratio >= 0.6) return 'bg-amber-500';
    return 'bg-emerald-500';
  };

  return (
    <div className="bg-[var(--surface)] border border-[var(--surface-border)] rounded-xl p-5 shadow-2xl space-y-5 max-w-4xl mx-auto w-full">
      {/* Top Header */}
      <div className="flex items-center justify-between border-b border-[var(--surface-border)] pb-3">
        <div className="flex items-center gap-2.5">
          <div className="w-8 h-8 rounded-lg bg-blue-500/10 border border-blue-500/30 flex items-center justify-center text-blue-400">
            <Gauge className="w-4 h-4" />
          </div>
          <div>
            <h2 className="text-sm font-semibold text-zinc-100 flex items-center gap-2">
              <span>Planner Rate-Limit Telemetry</span>
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/30">
                Live Rust Quota Manager
              </span>
            </h2>
            <p className="text-[11px] text-[var(--foreground-muted)]">
              Real-time sliding window RPM/TPM and daily RPD tracker with 80% threshold auto-cascade.
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {/* Pin/Unpin Gemini 3.8 Flash Button */}
          <button
            type="button"
            onClick={() => onPinModel(isGemini38Pinned ? null : 'gemini-3.8-flash')}
            disabled={isPinnedActionLoading}
            className={`px-3 py-1.5 rounded-lg text-xs font-medium flex items-center gap-1.5 transition-colors border shadow-sm ${
              isGemini38Pinned
                ? 'bg-purple-600 hover:bg-purple-500 text-white border-purple-500/40'
                : 'bg-[var(--surface-raised)] hover:bg-zinc-800 text-zinc-300 border-[var(--surface-border)]'
            }`}
          >
            <Pin className={`w-3.5 h-3.5 ${isGemini38Pinned ? 'fill-current' : ''}`} />
            <span>
              {isGemini38Pinned ? 'Pinned: Gemini 3.8 Flash' : 'Pin Gemini 3.8 Flash (Advanced)'}
            </span>
          </button>

          {onClose && (
            <button
              type="button"
              onClick={onClose}
              className="p-1.5 rounded-lg hover:bg-[var(--surface-raised)] text-zinc-400 hover:text-zinc-200 transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>

      {/* Switch Reason / Active Banner */}
      <div className="p-3 rounded-lg bg-[var(--surface-raised)] border border-[var(--surface-border)] flex items-center justify-between text-xs">
        <div className="flex items-center gap-2">
          <Activity className="w-4 h-4 text-emerald-400 shrink-0" />
          <span className="text-zinc-400">Active Routing:</span>
          <span className="font-mono text-zinc-200 font-semibold">{quotaState.active_model}</span>
          {isGemini38Pinned && (
            <span className="px-1.5 py-0.5 rounded bg-purple-500/20 text-purple-300 border border-purple-500/30 text-[10px]">
              Manual Pin (Auto-Switch Disabled)
            </span>
          )}
        </div>
        {quotaState.last_switch_reason && (
          <div className="text-[11px] text-zinc-400 italic max-w-md truncate text-right">
            {quotaState.last_switch_reason}
          </div>
        )}
      </div>

      {/* 4 Models Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {quotaState.models.map((model) => {
          const isActive = quotaState.active_model === model.model_id;
          const isPinnedThis = quotaState.pinned_model === model.model_id;

          return (
            <div
              key={model.model_id}
              className={`p-4 rounded-xl border space-y-3 transition-colors ${
                isActive
                  ? 'bg-zinc-900/90 border-blue-500/50 shadow-md ring-1 ring-blue-500/30'
                  : 'bg-[var(--surface-raised)] border-[var(--surface-border)] opacity-85 hover:opacity-100'
              }`}
            >
              {/* Model Header */}
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  {model.model_id.includes('3.8') ? (
                    <Sparkles className="w-4 h-4 text-purple-400" />
                  ) : model.model_id.includes('Gemma') ? (
                    <Layers className="w-4 h-4 text-emerald-400" />
                  ) : (
                    <Zap className="w-4 h-4 text-blue-400" />
                  )}
                  <span className="text-xs font-semibold text-zinc-200">
                    {model.display_name}
                  </span>
                </div>

                <div className="flex items-center gap-1.5">
                  {isPinnedThis && (
                    <span className="px-1.5 py-0.5 rounded bg-purple-500/20 text-purple-400 border border-purple-500/40 text-[9px] font-mono">
                      Pinned
                    </span>
                  )}
                  {isActive && (
                    <span className="px-2 py-0.5 rounded bg-blue-500/20 text-blue-400 border border-blue-500/40 text-[10px] font-mono animate-pulse">
                      Active Planner
                    </span>
                  )}
                  {model.threshold_80_percent_exceeded && (
                    <span className="px-1.5 py-0.5 rounded bg-rose-500/20 text-rose-400 border border-rose-500/40 text-[9px] font-mono">
                      ≥80% Thresh
                    </span>
                  )}
                  {!model.is_available && (
                    <span
                      className="px-1.5 py-0.5 rounded bg-amber-500/20 text-amber-400 border border-amber-500/40 text-[9px] font-mono flex items-center gap-1"
                      title={model.unavailability_reason || 'Model unavailable or cooling down'}
                    >
                      <AlertTriangle className="w-2.5 h-2.5" />
                      Unavailable
                    </span>
                  )}
                </div>
              </div>

              {/* Three Metrics: RPM, TPM, RPD */}
              <div className="space-y-2.5 text-xs">
                {/* 1. RPM Gauge */}
                <div className="space-y-1">
                  <div className="flex justify-between text-[11px]">
                    <span className="text-zinc-400">RPM (Reqs/Min):</span>
                    <span className="font-mono text-zinc-200">
                      {model.rpm_consumed} / {model.rpm_limit}{' '}
                      <span className="text-zinc-500 text-[10px]">
                        ({model.rpm_remaining} left, resets in {model.rpm_reset_seconds}s)
                      </span>
                    </span>
                  </div>
                  <div className="w-full h-1.5 rounded-full bg-zinc-800 overflow-hidden relative">
                    <div
                      className={`h-full transition-all duration-300 ${getMetricBarBg(
                        model.rpm_consumed,
                        model.rpm_limit
                      )}`}
                      style={{
                        width: `${Math.min(100, (model.rpm_consumed / model.rpm_limit) * 100)}%`,
                      }}
                    />
                    {/* 80% Threshold Marker */}
                    <div className="absolute top-0 bottom-0 left-[80%] w-0.5 bg-zinc-500/70" />
                  </div>
                </div>

                {/* 2. TPM Gauge */}
                <div className="space-y-1">
                  <div className="flex justify-between text-[11px]">
                    <span className="text-zinc-400">TPM (Tokens/Min):</span>
                    <span className="font-mono text-zinc-200">
                      {model.tpm_consumed.toLocaleString()} / {model.tpm_limit.toLocaleString()}{' '}
                      <span className="text-zinc-500 text-[10px]">
                        ({model.tpm_remaining.toLocaleString()} left)
                      </span>
                    </span>
                  </div>
                  <div className="w-full h-1.5 rounded-full bg-zinc-800 overflow-hidden relative">
                    <div
                      className={`h-full transition-all duration-300 ${getMetricBarBg(
                        model.tpm_consumed,
                        model.tpm_limit
                      )}`}
                      style={{
                        width: `${Math.min(100, (model.tpm_consumed / model.tpm_limit) * 100)}%`,
                      }}
                    />
                    {/* 80% Threshold Marker */}
                    <div className="absolute top-0 bottom-0 left-[80%] w-0.5 bg-zinc-500/70" />
                  </div>
                </div>

                {/* 3. RPD Gauge */}
                <div className="space-y-1">
                  <div className="flex justify-between text-[11px]">
                    <span className="text-zinc-400">RPD (Reqs/Day):</span>
                    <span className="font-mono text-zinc-200">
                      {model.rpd_consumed} / {model.rpd_limit}{' '}
                      <span className="text-zinc-500 text-[10px]">
                        ({model.rpd_remaining} left)
                      </span>
                    </span>
                  </div>
                  <div className="w-full h-1.5 rounded-full bg-zinc-800 overflow-hidden relative">
                    <div
                      className={`h-full transition-all duration-300 ${getMetricBarBg(
                        model.rpd_consumed,
                        model.rpd_limit
                      )}`}
                      style={{
                        width: `${Math.min(100, (model.rpd_consumed / model.rpd_limit) * 100)}%`,
                      }}
                    />
                    {/* 80% Threshold Marker */}
                    <div className="absolute top-0 bottom-0 left-[80%] w-0.5 bg-zinc-500/70" />
                  </div>
                  <div className="flex items-center gap-1 text-[10px] text-zinc-500 pt-0.5">
                    <Clock className="w-3 h-3" />
                    <span>RPD Rollover: {model.rpd_reset_formatted}</span>
                  </div>
                </div>
              </div>

              {model.unavailability_reason && (
                <div className="text-[10px] text-amber-300/90 bg-amber-500/10 border border-amber-500/30 rounded px-2.5 py-1.5 flex items-center gap-2 font-mono">
                  <AlertTriangle className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                  <span className="truncate" title={model.unavailability_reason}>
                    {model.unavailability_reason}
                  </span>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Footer Info / Rule Summary */}
      <div className="pt-2 border-t border-[var(--surface-border)] flex items-center justify-between text-[11px] text-[var(--foreground-muted)]">
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-emerald-400"></span>
          <span>
            Waterfall Cascade: 3.5 Flash-Lite (15 RPM) → 3.1 Flash-Lite (15 RPM) → Gemma 4 31B (30 RPM, 14.4k RPD)
          </span>
        </div>
        <div className="text-[10px] text-zinc-500 font-mono">
          Threshold Rule: Auto-switch at ≥80% on RPM, TPM, or RPD
        </div>
      </div>
    </div>
  );
};
