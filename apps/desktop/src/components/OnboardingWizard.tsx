import React, { useState, useEffect } from 'react';
import {
  Sparkles,
  Shield,
  CheckCircle2,
  AlertTriangle,
  ArrowRight,
  ArrowLeft,
  Loader2,
  ExternalLink,
  Eye,
  EyeOff,
  Cpu,
  Check,
  X,
} from 'lucide-react';
import {
  getGeminiKeyStatus,
  setGeminiApiKey,
  validateGeminiApiKey,
  getGroqKeyStatus,
  setGroqApiKey,
  validateGroqApiKey,
  verifyStartupDependencies,
} from '../lib/ipc';
import type { DependencyStatus, GeminiKeyStatus, GroqKeyStatus } from '../types';

interface OnboardingWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onComplete: () => void;
  initialStep?: 1 | 2 | 3 | 4;
}

export const OnboardingWizard: React.FC<OnboardingWizardProps> = ({
  isOpen,
  onClose,
  onComplete,
  initialStep = 1,
}) => {
  const [currentStep, setCurrentStep] = useState<number>(initialStep);

  // Gemini Key State
  const [geminiKeyInput, setGeminiKeyInput] = useState('');
  const [showGeminiKey, setShowGeminiKey] = useState(false);
  const [geminiStatus, setGeminiStatus] = useState<GeminiKeyStatus | null>(null);
  const [isValidatingGemini, setIsValidatingGemini] = useState(false);
  const [geminiError, setGeminiError] = useState<string | null>(null);
  const [geminiSuccess, setGeminiSuccess] = useState(false);

  // Groq Key State
  const [groqKeyInput, setGroqKeyInput] = useState('');
  const [showGroqKey, setShowGroqKey] = useState(false);
  const [groqStatus, setGroqStatus] = useState<GroqKeyStatus | null>(null);
  const [isValidatingGroq, setIsValidatingGroq] = useState(false);
  const [groqError, setGroqError] = useState<string | null>(null);
  const [groqSuccess, setGroqSuccess] = useState(false);

  // Dependencies State
  const [dependencies, setDependencies] = useState<DependencyStatus[]>([]);
  const [isLoadingDeps, setIsLoadingDeps] = useState(false);

  useEffect(() => {
    if (isOpen) {
      setCurrentStep(initialStep);
      loadStatuses();
    }
  }, [isOpen, initialStep]);

  const loadStatuses = async () => {
    try {
      const gStatus = await getGeminiKeyStatus();
      setGeminiStatus(gStatus);
      if (gStatus?.has_key) {
        setGeminiSuccess(true);
      }
    } catch (e) {
      console.warn('Failed to load Gemini key status:', e);
    }

    try {
      const grStatus = await getGroqKeyStatus();
      setGroqStatus(grStatus);
      if (grStatus?.has_key) {
        setGroqSuccess(true);
      }
    } catch (e) {
      console.warn('Failed to load Groq key status:', e);
    }
  };

  const loadDependencies = async () => {
    setIsLoadingDeps(true);
    try {
      const res = await verifyStartupDependencies();
      setDependencies(res?.dependencies || []);
    } catch (e) {
      console.warn('Failed to check dependencies:', e);
    } finally {
      setIsLoadingDeps(false);
    }
  };

  const handleValidateAndSaveGemini = async () => {
    const key = geminiKeyInput.trim();
    if (!key) {
      setGeminiError('Please enter a Gemini API key');
      return;
    }

    setIsValidatingGemini(true);
    setGeminiError(null);
    try {
      await validateGeminiApiKey(key);
      const updated = await setGeminiApiKey(key);
      setGeminiStatus(updated);
      setGeminiSuccess(true);
      setGeminiKeyInput('');
      // Advance to step 3 after 600ms
      setTimeout(() => {
        setCurrentStep(3);
      }, 600);
    } catch (err: any) {
      setGeminiError(typeof err === 'string' ? err : err.message || 'Key validation failed');
      setGeminiSuccess(false);
    } finally {
      setIsValidatingGemini(false);
    }
  };

  const handleValidateAndSaveGroq = async () => {
    const key = groqKeyInput.trim();
    if (!key) {
      setGroqError('Please enter a Groq API key');
      return;
    }

    setIsValidatingGroq(true);
    setGroqError(null);
    try {
      await validateGroqApiKey(key);
      const updated = await setGroqApiKey(key);
      setGroqStatus(updated);
      setGroqSuccess(true);
      setGroqKeyInput('');
      // Advance to step 4 after 600ms
      setTimeout(() => {
        loadDependencies();
        setCurrentStep(4);
      }, 600);
    } catch (err: any) {
      setGroqError(typeof err === 'string' ? err : err.message || 'Groq key validation failed');
      setGroqSuccess(false);
    } finally {
      setIsValidatingGroq(false);
    }
  };

  const handleSkipGroq = () => {
    loadDependencies();
    setCurrentStep(4);
  };

  const handleFinish = () => {
    try {
      localStorage.setItem('zencode_onboarding_completed', 'true');
    } catch {}
    onComplete();
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-md p-4 select-none font-mono">
      <div className="w-full max-w-xl bg-[#121214] border border-[#27272a] rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
        {/* Top Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-[#222225] bg-[#161618]">
          <div className="flex items-center gap-2.5">
            <div className="w-6 h-6 rounded-md bg-[#27272a] flex items-center justify-center text-xs font-bold text-white">
              Z
            </div>
            <span className="text-sm font-semibold text-zinc-100">ZenCode Setup</span>
          </div>

          {/* Step indicator */}
          <div className="flex items-center gap-2">
            {[1, 2, 3, 4].map((step) => (
              <div
                key={step}
                className={`w-2 h-2 rounded-full transition-colors ${
                  currentStep === step
                    ? 'bg-[#38bdf8]'
                    : currentStep > step
                    ? 'bg-zinc-500'
                    : 'bg-zinc-700'
                }`}
              />
            ))}
            <button
              type="button"
              onClick={onClose}
              className="ml-3 text-zinc-500 hover:text-zinc-200 transition-colors"
              title="Close"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Content Body */}
        <div className="p-6 overflow-y-auto space-y-6 flex-1 text-xs">
          {/* STEP 1: Welcome & Architecture */}
          {currentStep === 1 && (
            <div className="space-y-5">
              <div>
                <h2 className="text-base font-semibold text-zinc-100">Welcome to ZenCode</h2>
                <p className="text-zinc-400 mt-1 leading-relaxed">
                  A high-velocity agentic coding environment where all file changes are executed strictly in isolated Git worktrees.
                </p>
              </div>

              <div className="grid grid-cols-1 gap-3 pt-2">
                <div className="p-3.5 rounded-xl bg-[#18181b] border border-[#27272a] space-y-2">
                  <div className="flex items-center gap-2 text-zinc-200 font-semibold">
                    <Sparkles className="w-4 h-4 text-[#38bdf8]" />
                    <span>Director (Gemini Planning)</span>
                  </div>
                  <p className="text-zinc-400 text-[11px] leading-relaxed">
                    Deconstructs prompts into structured implementation steps with explicit context previews. Requires a free Gemini API key from Google.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-[#18181b] border border-[#27272a] space-y-2">
                  <div className="flex items-center gap-2 text-zinc-200 font-semibold">
                    <Cpu className="w-4 h-4 text-zinc-300" />
                    <span>Worker Agents (Local &amp; Cloud)</span>
                  </div>
                  <p className="text-zinc-400 text-[11px] leading-relaxed">
                    Executes edits locally via <span className="text-zinc-200">Ollama</span> by default. You can also configure <span className="text-zinc-200">Groq (BYOK)</span> for fast cloud open-source models with diff-only outputs.
                  </p>
                </div>

                <div className="p-3.5 rounded-xl bg-[#18181b] border border-[#27272a] space-y-2">
                  <div className="flex items-center gap-2 text-zinc-200 font-semibold">
                    <Shield className="w-4 h-4 text-emerald-400" />
                    <span>Windows Credential Manager Security</span>
                  </div>
                  <p className="text-zinc-400 text-[11px] leading-relaxed">
                    API keys are stored strictly in the Windows Credential Manager vault—never written to repository files, env vars, or client memory.
                  </p>
                </div>
              </div>

              <div className="pt-4 flex justify-end">
                <button
                  type="button"
                  onClick={() => setCurrentStep(2)}
                  className="px-4 py-2 rounded-lg bg-[#38bdf8] text-[#0a0a0a] font-semibold flex items-center gap-2 hover:bg-[#0284c7] transition-colors"
                >
                  <span>Get Started</span>
                  <ArrowRight className="w-4 h-4" />
                </button>
              </div>
            </div>
          )}

          {/* STEP 2: Gemini API Key */}
          {currentStep === 2 && (
            <div className="space-y-5">
              <div>
                <div className="flex items-center gap-2">
                  <span className="text-[10px] font-bold text-[#38bdf8] uppercase tracking-wider bg-[#1a2333] border border-[#38bdf8]/30 px-2 py-0.5 rounded">
                    Required
                  </span>
                  <h2 className="text-base font-semibold text-zinc-100">Gemini Director Key</h2>
                </div>
                <p className="text-zinc-400 mt-1 leading-relaxed">
                  ZenCode uses Gemini 3.5 Flash Lite exclusively for planning, context previews, and advisory repair guidance.
                </p>
              </div>

              {/* Status if already configured */}
              {geminiStatus?.has_key && (
                <div className="p-3 rounded-lg bg-[#18181b] border border-emerald-900/40 text-emerald-400 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-4 h-4 shrink-0" />
                    <span>Key is already configured ({geminiStatus.masked_key})</span>
                  </div>
                  <button
                    type="button"
                    onClick={() => setCurrentStep(3)}
                    className="text-[11px] underline hover:text-white"
                  >
                    Keep &amp; Continue
                  </button>
                </div>
              )}

              {/* Input Form */}
              <div className="space-y-3 p-4 bg-[#18181b] border border-[#27272a] rounded-xl">
                <div className="flex items-center justify-between">
                  <label className="text-zinc-200 font-medium">Gemini API Key</label>
                  <a
                    href="https://aistudio.google.com/app/apikey"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-[#38bdf8] hover:underline flex items-center gap-1 text-[11px]"
                  >
                    <span>Get free key from Google AI Studio</span>
                    <ExternalLink className="w-3 h-3" />
                  </a>
                </div>

                <div className="relative">
                  <input
                    type={showGeminiKey ? 'text' : 'password'}
                    value={geminiKeyInput}
                    onChange={(e) => setGeminiKeyInput(e.target.value)}
                    placeholder="AIza..."
                    className="w-full bg-[#121214] border border-[#2a2a2e] focus:border-[#38bdf8] rounded-lg px-3 py-2 text-xs text-white placeholder-zinc-600 focus:outline-none font-mono"
                  />
                  <button
                    type="button"
                    onClick={() => setShowGeminiKey(!showGeminiKey)}
                    className="absolute right-2.5 top-2.5 text-zinc-500 hover:text-zinc-300"
                  >
                    {showGeminiKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                  </button>
                </div>

                {geminiError && (
                  <div className="p-2.5 rounded bg-rose-950/30 border border-rose-900/60 text-rose-400 flex items-start gap-2 text-[11px]">
                    <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
                    <span>{geminiError}</span>
                  </div>
                )}

                {geminiSuccess && (
                  <div className="p-2.5 rounded bg-emerald-950/30 border border-emerald-900/60 text-emerald-400 flex items-center gap-2 text-[11px]">
                    <CheckCircle2 className="w-4 h-4 shrink-0" />
                    <span>Key verified and saved to Windows Credential Manager!</span>
                  </div>
                )}
              </div>

              <div className="pt-4 flex items-center justify-between">
                <button
                  type="button"
                  onClick={() => setCurrentStep(1)}
                  className="px-3 py-1.5 rounded-lg border border-[#27272a] text-zinc-400 hover:text-white flex items-center gap-1.5"
                >
                  <ArrowLeft className="w-3.5 h-3.5" />
                  <span>Back</span>
                </button>

                <div className="flex items-center gap-2">
                  {geminiStatus?.has_key && (
                    <button
                      type="button"
                      onClick={() => setCurrentStep(3)}
                      className="px-3.5 py-1.5 rounded-lg border border-[#27272a] text-zinc-300 hover:text-white"
                    >
                      Next
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={handleValidateAndSaveGemini}
                    disabled={!geminiKeyInput.trim() || isValidatingGemini}
                    className="px-4 py-2 rounded-lg bg-[#38bdf8] text-[#0a0a0a] font-semibold flex items-center gap-2 hover:bg-[#0284c7] disabled:opacity-40 transition-colors"
                  >
                    {isValidatingGemini ? (
                      <>
                        <Loader2 className="w-4 h-4 animate-spin" />
                        <span>Verifying with Google...</span>
                      </>
                    ) : (
                      <>
                        <span>Validate &amp; Save</span>
                        <ArrowRight className="w-4 h-4" />
                      </>
                    )}
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* STEP 3: Groq API Key (Optional) */}
          {currentStep === 3 && (
            <div className="space-y-5">
              <div>
                <div className="flex items-center gap-2">
                  <span className="text-[10px] font-bold text-zinc-400 uppercase tracking-wider bg-zinc-800 border border-zinc-700 px-2 py-0.5 rounded">
                    Optional
                  </span>
                  <h2 className="text-base font-semibold text-zinc-100">Groq Cloud Worker (BYOK)</h2>
                </div>
                <p className="text-zinc-400 mt-1 leading-relaxed">
                  Local Ollama execution is the default. Connect your Groq API key if you want cloud worker rotation (gpt-oss-120b, gpt-oss-20b, qwen3.8-27b) with diff-only output.
                </p>
              </div>

              {/* Status if already configured */}
              {groqStatus?.has_key && (
                <div className="p-3 rounded-lg bg-[#18181b] border border-emerald-900/40 text-emerald-400 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-4 h-4 shrink-0" />
                    <span>Groq Key configured ({groqStatus.masked_key})</span>
                  </div>
                  <button
                    type="button"
                    onClick={() => {
                      loadDependencies();
                      setCurrentStep(4);
                    }}
                    className="text-[11px] underline hover:text-white"
                  >
                    Keep &amp; Continue
                  </button>
                </div>
              )}

              {/* Input Form */}
              <div className="space-y-3 p-4 bg-[#18181b] border border-[#27272a] rounded-xl">
                <div className="flex items-center justify-between">
                  <label className="text-zinc-200 font-medium">Groq API Key</label>
                  <a
                    href="https://console.groq.com/keys"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-[#38bdf8] hover:underline flex items-center gap-1 text-[11px]"
                  >
                    <span>Get a key from Groq Console</span>
                    <ExternalLink className="w-3 h-3" />
                  </a>
                </div>

                <div className="relative">
                  <input
                    type={showGroqKey ? 'text' : 'password'}
                    value={groqKeyInput}
                    onChange={(e) => setGroqKeyInput(e.target.value)}
                    placeholder="gsk_..."
                    className="w-full bg-[#121214] border border-[#2a2a2e] focus:border-[#38bdf8] rounded-lg px-3 py-2 text-xs text-white placeholder-zinc-600 focus:outline-none font-mono"
                  />
                  <button
                    type="button"
                    onClick={() => setShowGroqKey(!showGroqKey)}
                    className="absolute right-2.5 top-2.5 text-zinc-500 hover:text-zinc-300"
                  >
                    {showGroqKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                  </button>
                </div>

                {groqError && (
                  <div className="p-2.5 rounded bg-rose-950/30 border border-rose-900/60 text-rose-400 flex items-start gap-2 text-[11px]">
                    <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
                    <span>{groqError}</span>
                  </div>
                )}

                {groqSuccess && (
                  <div className="p-2.5 rounded bg-emerald-950/30 border border-emerald-900/60 text-emerald-400 flex items-center gap-2 text-[11px]">
                    <CheckCircle2 className="w-4 h-4 shrink-0" />
                    <span>Groq Key verified and saved to Windows Credential Manager!</span>
                  </div>
                )}
              </div>

              <div className="pt-4 flex items-center justify-between">
                <button
                  type="button"
                  onClick={() => setCurrentStep(2)}
                  className="px-3 py-1.5 rounded-lg border border-[#27272a] text-zinc-400 hover:text-white flex items-center gap-1.5"
                >
                  <ArrowLeft className="w-3.5 h-3.5" />
                  <span>Back</span>
                </button>

                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={handleSkipGroq}
                    className="px-3.5 py-1.5 rounded-lg border border-[#27272a] text-zinc-400 hover:text-white transition-colors"
                  >
                    Set up later (Local Only)
                  </button>

                  <button
                    type="button"
                    onClick={handleValidateAndSaveGroq}
                    disabled={!groqKeyInput.trim() || isValidatingGroq}
                    className="px-4 py-2 rounded-lg bg-[#38bdf8] text-[#0a0a0a] font-semibold flex items-center gap-2 hover:bg-[#0284c7] disabled:opacity-40 transition-colors"
                  >
                    {isValidatingGroq ? (
                      <>
                        <Loader2 className="w-4 h-4 animate-spin" />
                        <span>Verifying with Groq...</span>
                      </>
                    ) : (
                      <>
                        <span>Validate &amp; Save</span>
                        <ArrowRight className="w-4 h-4" />
                      </>
                    )}
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* STEP 4: Dependencies Readiness */}
          {currentStep === 4 && (
            <div className="space-y-5">
              <div>
                <h2 className="text-base font-semibold text-zinc-100">Local Dependency Verification</h2>
                <p className="text-zinc-400 mt-1 leading-relaxed">
                  Confirming your local toolchain is ready for Git-isolated autonomous worker sessions.
                </p>
              </div>

              {isLoadingDeps ? (
                <div className="p-8 flex flex-col items-center justify-center space-y-2 text-zinc-400">
                  <Loader2 className="w-6 h-6 animate-spin text-[#38bdf8]" />
                  <span>Running dependency health checks...</span>
                </div>
              ) : (
                <div className="space-y-2.5">
                  {dependencies.map((dep, idx) => (
                    <div
                      key={idx}
                      className="p-3 rounded-xl bg-[#18181b] border border-[#27272a] flex items-start justify-between gap-3"
                    >
                      <div className="space-y-1">
                        <div className="flex items-center gap-2">
                          <span className="font-semibold text-zinc-200">{dep.name}</span>
                          <span
                            className={`text-[10px] px-1.5 py-0.5 rounded font-mono ${
                              dep.is_ready
                                ? 'bg-emerald-950/50 text-emerald-400 border border-emerald-900/40'
                                : 'bg-amber-950/50 text-amber-400 border border-amber-900/40'
                            }`}
                          >
                            {dep.is_ready ? 'Ready' : 'Attention Needed'}
                          </span>
                        </div>
                        <p className="text-zinc-400 text-[11px]">{dep.details}</p>
                        {dep.fix_recommendation && !dep.is_ready && (
                          <p className="text-amber-400 text-[10px] font-mono mt-1">
                            &bull; {dep.fix_recommendation}
                          </p>
                        )}
                      </div>

                      <div className="pt-0.5">
                        {dep.is_ready ? (
                          <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
                        ) : (
                          <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0" />
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              )}

              <div className="pt-4 flex items-center justify-between">
                <button
                  type="button"
                  onClick={() => setCurrentStep(3)}
                  className="px-3 py-1.5 rounded-lg border border-[#27272a] text-zinc-400 hover:text-white flex items-center gap-1.5"
                >
                  <ArrowLeft className="w-3.5 h-3.5" />
                  <span>Back</span>
                </button>

                <button
                  type="button"
                  onClick={handleFinish}
                  className="px-5 py-2 rounded-lg bg-[#38bdf8] text-[#0a0a0a] font-semibold flex items-center gap-2 hover:bg-[#0284c7] transition-colors shadow-md"
                >
                  <span>Launch ZenCode</span>
                  <Check className="w-4 h-4" />
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
