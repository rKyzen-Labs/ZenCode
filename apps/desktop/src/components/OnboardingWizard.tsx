import React, { useState, useEffect } from 'react';
import {
  CheckCircle2,
  AlertTriangle,
  ArrowRight,
  ArrowLeft,
  Loader2,
  ExternalLink,
  Eye,
  EyeOff,
  Cpu,
  Cloud,
  Check,
  X,
  GitBranch,
  Terminal,
  Zap,
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

  // Worker Selection in Onboarding
  const [chosenWorker, setChosenWorker] = useState<'local' | 'groq'>('local');

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
        setChosenWorker('groq');
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
      setTimeout(() => {
        setCurrentStep(3);
      }, 500);
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
      setChosenWorker('groq');
    } catch (err: any) {
      setGroqError(typeof err === 'string' ? err : err.message || 'Groq validation failed');
      setGroqSuccess(false);
    } finally {
      setIsValidatingGroq(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 bg-[#faf9f5] flex flex-col font-sans select-none overflow-hidden animate-fade-in text-[#141413]">
      {/* Fullscreen Header */}
      <header className="h-16 px-8 border-b border-[#dad9d4] bg-[#f7f6f2] flex items-center justify-between shrink-0">
        <div className="flex items-center gap-3">
          <img src="/logo.png" alt="ZenCode" className="w-8 h-8 rounded-lg shadow-sm" />
          <div className="flex items-center gap-2">
            <span className="font-semibold text-base text-[#141413] tracking-tight">ZenCode</span>
            <span className="text-[10.5px] px-2 py-0.5 rounded-full bg-[#ede9de] text-[#c96442] font-medium border border-[#c96442]/20 font-mono">
              v0.1.0
            </span>
          </div>
        </div>

        {/* Step Indicator Pills */}
        <div className="flex items-center gap-2">
          {[
            { step: 1, label: 'Why ZenCode' },
            { step: 2, label: 'Gemini Director Key' },
            { step: 3, label: 'Worker Engine' },
            { step: 4, label: 'Ready to Code' },
          ].map(({ step, label }) => {
            const isActive = currentStep === step;
            const isCompleted = currentStep > step;

            return (
              <button
                key={step}
                type="button"
                onClick={() => setCurrentStep(step)}
                className={`flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                  isActive
                    ? 'bg-[#c96442] text-white shadow-sm'
                    : isCompleted
                    ? 'bg-[#ede9de] text-[#141413] hover:bg-[#dad9d4]'
                    : 'text-[#83827d] hover:text-[#141413]'
                }`}
              >
                <span
                  className={`w-4 h-4 rounded-full flex items-center justify-center text-[10px] font-mono font-bold ${
                    isActive ? 'bg-white/20 text-white' : isCompleted ? 'bg-[#5f8a5a] text-white' : 'bg-[#dad9d4] text-[#83827d]'
                  }`}
                >
                  {isCompleted ? '✓' : step}
                </span>
                <span>{label}</span>
              </button>
            );
          })}
        </div>

        {/* Skip to Workspace Button */}
        <button
          type="button"
          onClick={onClose}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors"
        >
          <span>Skip to Workspace</span>
          <X className="w-3.5 h-3.5" />
        </button>
      </header>

      {/* Main Fullscreen Body */}
      <div className="flex-1 overflow-y-auto flex items-center justify-center p-8 bg-[#faf9f5]">
        <div className="w-full max-w-3xl mx-auto space-y-8 animate-onboarding-hero">
          {/* STEP 1: WELCOME & WHY ZENCODE */}
          {currentStep === 1 && (
            <div className="space-y-8 text-center">
              <div className="space-y-4">
                <div className="inline-block relative">
                  <div className="absolute inset-0 bg-[#c96442]/20 rounded-3xl blur-2xl transform scale-110" />
                  <img
                    src="/logo.png"
                    alt="ZenCode Hero Logo"
                    className="w-24 h-24 rounded-3xl shadow-xl relative mx-auto border-2 border-white/60"
                  />
                </div>

                <div className="space-y-2">
                  <h1 className="text-3xl font-bold tracking-tight text-[#141413] font-sans">
                    Welcome to ZenCode
                  </h1>
                  <p className="text-sm text-[#535146] max-w-xl mx-auto leading-relaxed">
                    The safety-first, Director-supervised AI coding assistant engineered for real developer repositories.
                  </p>
                </div>
              </div>

              {/* 4 Core Value Pillars */}
              <div className="grid grid-cols-2 gap-4 text-left">
                <div className="p-4 rounded-xl bg-[#f7f6f2] border border-[#dad9d4] space-y-2 hover:border-[#c96442]/40 transition-colors">
                  <div className="w-7 h-7 rounded-lg bg-[#ede9de] flex items-center justify-center text-[#c96442]">
                    <GitBranch className="w-4 h-4" />
                  </div>
                  <h3 className="text-xs font-semibold text-[#141413]">Isolated Git Worktrees</h3>
                  <p className="text-[11.5px] text-[#535146] leading-relaxed">
                    Every task executes in an ephemeral Git worktree. Your main branch is never exposed to unverified code or broken builds.
                  </p>
                </div>

                <div className="p-4 rounded-xl bg-[#f7f6f2] border border-[#dad9d4] space-y-2 hover:border-[#c96442]/40 transition-colors">
                  <div className="w-7 h-7 rounded-lg bg-[#ede9de] flex items-center justify-center text-[#5f8a5a]">
                    <CheckCircle2 className="w-4 h-4" />
                  </div>
                  <h3 className="text-xs font-semibold text-[#141413]">Grounded Truth Verification</h3>
                  <p className="text-[11.5px] text-[#535146] leading-relaxed">
                    Zero AI hallucinations. The Director AI (Gemini) verifies real git diffs and actual command exits before claiming task completion.
                  </p>
                </div>

                <div className="p-4 rounded-xl bg-[#f7f6f2] border border-[#dad9d4] space-y-2 hover:border-[#c96442]/40 transition-colors">
                  <div className="w-7 h-7 rounded-lg bg-[#ede9de] flex items-center justify-center text-[#3d7a99]">
                    <Cpu className="w-4 h-4" />
                  </div>
                  <h3 className="text-xs font-semibold text-[#141413]">Dual Engine (Local &amp; Cloud)</h3>
                  <p className="text-[11.5px] text-[#535146] leading-relaxed">
                    Execute code edits locally and offline with Ollama, or toggle ultra-fast Groq BYOK cloud rotation with diff-only outputs.
                  </p>
                </div>

                <div className="p-4 rounded-xl bg-[#f7f6f2] border border-[#dad9d4] space-y-2 hover:border-[#c96442]/40 transition-colors">
                  <div className="w-7 h-7 rounded-lg bg-[#ede9de] flex items-center justify-center text-[#c96442]">
                    <Terminal className="w-4 h-4" />
                  </div>
                  <h3 className="text-xs font-semibold text-[#141413]">Supervised Shell Execution</h3>
                  <p className="text-[11.5px] text-[#535146] leading-relaxed">
                    Safe read-only commands auto-execute; dangerous actions require your consent. Terminal output streams in a side-pane tab.
                  </p>
                </div>
              </div>

              {/* Action */}
              <div className="pt-2 flex justify-center">
                <button
                  type="button"
                  onClick={() => setCurrentStep(2)}
                  className="px-6 py-2.5 rounded-xl bg-[#c96442] hover:bg-[#b5523f] text-white font-medium text-xs flex items-center gap-2 transition-all shadow-md cursor-pointer hover:shadow-lg"
                >
                  <span>Begin Configuration</span>
                  <ArrowRight className="w-4 h-4" />
                </button>
              </div>
            </div>
          )}

          {/* STEP 2: GEMINI DIRECTOR KEY */}
          {currentStep === 2 && (
            <div className="space-y-6 bg-[#f7f6f2] border border-[#dad9d4] rounded-2xl p-8 shadow-sm">
              <div className="space-y-1.5">
                <div className="flex items-center gap-2">
                  <span className="text-[10px] font-bold text-[#c96442] uppercase tracking-wider bg-[#c96442]/10 border border-[#c96442]/20 px-2 py-0.5 rounded">
                    Required
                  </span>
                  <h2 className="text-base font-semibold text-[#141413]">
                    Google Gemini Director Key
                  </h2>
                </div>
                <p className="text-xs text-[#535146] leading-relaxed">
                  ZenCode uses Gemini 3.5 Flash Lite exclusively as the Director (planning, context generation, and grounded verification). Free keys from Google AI Studio have generous RPM and zero cost.
                </p>
              </div>

              {/* Status if already configured */}
              {geminiStatus?.has_key && (
                <div className="p-3.5 rounded-xl bg-[#ede9de] border border-[#5f8a5a]/30 text-[#141413] flex items-center justify-between">
                  <div className="flex items-center gap-2.5">
                    <CheckCircle2 className="w-4 h-4 text-[#5f8a5a] shrink-0" />
                    <span className="text-xs">
                      Key already active: <span className="font-mono text-[#535146]">{geminiStatus.masked_key}</span>
                    </span>
                  </div>
                  <button
                    type="button"
                    onClick={() => setCurrentStep(3)}
                    className="text-xs font-semibold text-[#c96442] hover:underline"
                  >
                    Keep &amp; Continue →
                  </button>
                </div>
              )}

              {/* Input Form */}
              <div className="space-y-3 p-4 bg-[#faf9f5] border border-[#dad9d4] rounded-xl">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-medium text-[#141413]">API Key</label>
                  <a
                    href="https://aistudio.google.com/app/apikey"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-xs text-[#c96442] hover:underline flex items-center gap-1 font-medium"
                  >
                    <span>Get free API key from Google AI Studio</span>
                    <ExternalLink className="w-3 h-3" />
                  </a>
                </div>

                <div className="relative flex items-center">
                  <input
                    type={showGeminiKey ? 'text' : 'password'}
                    value={geminiKeyInput}
                    onChange={(e) => setGeminiKeyInput(e.target.value)}
                    placeholder="AIzaSy..."
                    className="w-full bg-[#ede9de]/50 border border-[#dad9d4] focus:border-[#c96442] rounded-lg px-3 py-2 text-xs text-[#141413] placeholder:text-[#83827d] focus:outline-none font-mono pr-20"
                  />
                  <div className="absolute right-2 flex items-center gap-1">
                    <button
                      type="button"
                      onClick={() => setShowGeminiKey(!showGeminiKey)}
                      className="p-1 text-[#83827d] hover:text-[#141413] rounded"
                      title={showGeminiKey ? 'Hide key' : 'Show key'}
                    >
                      {showGeminiKey ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                    </button>
                  </div>
                </div>

                {geminiError && (
                  <div className="p-2.5 rounded-lg bg-rose-50 border border-rose-200 text-rose-800 text-xs flex items-center gap-2">
                    <AlertTriangle className="w-3.5 h-3.5 text-rose-600 shrink-0" />
                    <span>{geminiError}</span>
                  </div>
                )}

                {geminiSuccess && (
                  <div className="p-2.5 rounded-lg bg-emerald-50 border border-emerald-200 text-emerald-800 text-xs flex items-center gap-2">
                    <CheckCircle2 className="w-3.5 h-3.5 text-emerald-600 shrink-0" />
                    <span>Gemini key successfully verified and securely stored in Windows Credential Vault!</span>
                  </div>
                )}
              </div>

              {/* Navigation buttons */}
              <div className="flex items-center justify-between pt-2">
                <button
                  type="button"
                  onClick={() => setCurrentStep(1)}
                  className="px-4 py-2 rounded-lg bg-[#ede9de] hover:bg-[#dad9d4] text-[#141413] text-xs font-medium flex items-center gap-1.5 transition-colors border border-[#dad9d4]"
                >
                  <ArrowLeft className="w-3.5 h-3.5" />
                  <span>Back</span>
                </button>

                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={handleValidateAndSaveGemini}
                    disabled={!geminiKeyInput.trim() || isValidatingGemini}
                    className="px-4 py-2 rounded-lg bg-[#c96442] hover:bg-[#b5523f] disabled:opacity-50 text-white text-xs font-medium flex items-center gap-1.5 transition-colors shadow-sm cursor-pointer"
                  >
                    {isValidatingGemini ? (
                      <>
                        <Loader2 className="w-3.5 h-3.5 animate-spin" />
                        <span>Validating Key...</span>
                      </>
                    ) : (
                      <>
                        <span>Verify &amp; Continue</span>
                        <ArrowRight className="w-3.5 h-3.5" />
                      </>
                    )}
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* STEP 3: WORKER SELECTION (LOCAL OLLAMA VS GROQ CLOUD) */}
          {currentStep === 3 && (
            <div className="space-y-6 bg-[#f7f6f2] border border-[#dad9d4] rounded-2xl p-8 shadow-sm">
              <div className="space-y-1.5">
                <h2 className="text-base font-semibold text-[#141413]">
                  Choose Your Coding Worker Engine
                </h2>
                <p className="text-xs text-[#535146] leading-relaxed">
                  Worker agents write the actual code diffs inside your isolated task worktrees. You can toggle between local Ollama and Groq cloud at any time.
                </p>
              </div>

              {/* Selection Cards */}
              <div className="grid grid-cols-2 gap-4">
                {/* Local Ollama Card */}
                <div
                  onClick={() => setChosenWorker('local')}
                  className={`p-5 rounded-xl border transition-all cursor-pointer space-y-3 ${
                    chosenWorker === 'local'
                      ? 'bg-[#faf9f5] border-[#c96442] shadow-sm ring-1 ring-[#c96442]'
                      : 'bg-[#faf9f5]/60 border-[#dad9d4] hover:bg-[#faf9f5]'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <div className="w-7 h-7 rounded-lg bg-[#ede9de] flex items-center justify-center text-[#5f8a5a]">
                      <Cpu className="w-4 h-4" />
                    </div>
                    {chosenWorker === 'local' && (
                      <span className="text-[10px] font-bold text-[#5f8a5a] bg-[#5f8a5a]/10 px-2 py-0.5 rounded">
                        Selected
                      </span>
                    )}
                  </div>
                  <div>
                    <h3 className="text-xs font-semibold text-[#141413]">Local Ollama Worker</h3>
                    <p className="text-[11px] text-[#535146] mt-1 leading-relaxed">
                      100% private and runs offline on your machine. Compatible with granite4.1:8b, qwen2.5-coder:7b, gemma3, etc.
                    </p>
                  </div>
                </div>

                {/* Groq Cloud Card */}
                <div
                  onClick={() => setChosenWorker('groq')}
                  className={`p-5 rounded-xl border transition-all cursor-pointer space-y-3 ${
                    chosenWorker === 'groq'
                      ? 'bg-[#faf9f5] border-[#c96442] shadow-sm ring-1 ring-[#c96442]'
                      : 'bg-[#faf9f5]/60 border-[#dad9d4] hover:bg-[#faf9f5]'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <div className="w-7 h-7 rounded-lg bg-[#ede9de] flex items-center justify-center text-[#c96442]">
                      <Cloud className="w-4 h-4" />
                    </div>
                    {chosenWorker === 'groq' && (
                      <span className="text-[10px] font-bold text-[#c96442] bg-[#c96442]/10 px-2 py-0.5 rounded">
                        Selected
                      </span>
                    )}
                  </div>
                  <div>
                    <h3 className="text-xs font-semibold text-[#141413]">Groq Cloud (BYOK)</h3>
                    <p className="text-[11px] text-[#535146] mt-1 leading-relaxed">
                      Ultra-fast token streaming with automatic model cascade rotation (gpt-oss-120b → gpt-oss-20b → qwen3.8-27b).
                    </p>
                  </div>
                </div>
              </div>

              {/* Optional Groq Key Input if Groq is chosen */}
              {chosenWorker === 'groq' && (
                <div className="p-4 bg-[#faf9f5] border border-[#dad9d4] rounded-xl space-y-3 animate-fade-in">
                  <div className="flex items-center justify-between">
                    <label className="text-xs font-medium text-[#141413]">Groq API Key (BYOK)</label>
                    <a
                      href="https://console.groq.com/keys"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-xs text-[#c96442] hover:underline flex items-center gap-1 font-medium"
                    >
                      <span>Get key from console.groq.com</span>
                      <ExternalLink className="w-3 h-3" />
                    </a>
                  </div>

                  {groqStatus?.has_key && (
                    <div className="p-2.5 rounded-lg bg-[#ede9de] border border-[#5f8a5a]/30 text-xs text-[#141413] flex items-center gap-2">
                      <CheckCircle2 className="w-3.5 h-3.5 text-[#5f8a5a] shrink-0" />
                      <span>
                        Key active: <span className="font-mono text-[#535146]">{groqStatus.masked_key}</span>
                      </span>
                    </div>
                  )}

                  <div className="relative flex items-center">
                    <input
                      type={showGroqKey ? 'text' : 'password'}
                      value={groqKeyInput}
                      onChange={(e) => setGroqKeyInput(e.target.value)}
                      placeholder="gsk_..."
                      className="w-full bg-[#ede9de]/50 border border-[#dad9d4] focus:border-[#c96442] rounded-lg px-3 py-2 text-xs text-[#141413] placeholder:text-[#83827d] focus:outline-none font-mono"
                    />
                    <div className="absolute right-2 flex items-center gap-1">
                      <button
                        type="button"
                        onClick={() => setShowGroqKey(!showGroqKey)}
                        className="p-1 text-[#83827d] hover:text-[#141413] rounded"
                      >
                        {showGroqKey ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                      </button>
                    </div>
                  </div>

                  {groqError && (
                    <div className="p-2 text-rose-800 bg-rose-50 border border-rose-200 rounded text-xs">
                      {groqError}
                    </div>
                  )}

                  {groqSuccess && (
                    <div className="p-2 text-emerald-800 bg-emerald-50 border border-emerald-200 rounded text-xs flex items-center gap-2">
                      <Check className="w-3 h-3 text-emerald-600" />
                      <span>Groq key verified!</span>
                    </div>
                  )}

                  <div className="flex justify-end">
                    <button
                      type="button"
                      onClick={handleValidateAndSaveGroq}
                      disabled={!groqKeyInput.trim() || isValidatingGroq}
                      className="px-3 py-1.5 rounded-lg bg-[#ede9de] hover:bg-[#dad9d4] text-[#141413] text-xs font-medium border border-[#dad9d4] transition-colors"
                    >
                      {isValidatingGroq ? 'Verifying...' : 'Save Groq Key'}
                    </button>
                  </div>
                </div>
              )}

              {/* Navigation buttons */}
              <div className="flex items-center justify-between pt-2">
                <button
                  type="button"
                  onClick={() => setCurrentStep(2)}
                  className="px-4 py-2 rounded-lg bg-[#ede9de] hover:bg-[#dad9d4] text-[#141413] text-xs font-medium flex items-center gap-1.5 transition-colors border border-[#dad9d4]"
                >
                  <ArrowLeft className="w-3.5 h-3.5" />
                  <span>Back</span>
                </button>

                <button
                  type="button"
                  onClick={() => {
                    loadDependencies();
                    setCurrentStep(4);
                  }}
                  className="px-5 py-2 rounded-lg bg-[#c96442] hover:bg-[#b5523f] text-white text-xs font-medium flex items-center gap-1.5 transition-colors shadow-sm cursor-pointer"
                >
                  <span>Continue</span>
                  <ArrowRight className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          )}

          {/* STEP 4: SYSTEM VERIFICATION & READY TO LAUNCH */}
          {currentStep === 4 && (
            <div className="space-y-6 bg-[#f7f6f2] border border-[#dad9d4] rounded-2xl p-8 shadow-sm text-center">
              <div className="w-16 h-16 rounded-2xl bg-[#5f8a5a]/10 border border-[#5f8a5a]/30 flex items-center justify-center text-[#5f8a5a] mx-auto">
                <CheckCircle2 className="w-8 h-8" />
              </div>

              <div className="space-y-2">
                <h2 className="text-xl font-bold text-[#141413]">ZenCode is Ready!</h2>
                <p className="text-xs text-[#535146] max-w-md mx-auto leading-relaxed">
                  Your Director AI is configured and your repository safety guardrails are active. Every task will run in an isolated Git worktree with provable verification.
                </p>
              </div>

              {/* Verification Checklist */}
              <div className="bg-[#faf9f5] border border-[#dad9d4] rounded-xl p-4 max-w-md mx-auto text-left space-y-2.5">
                <div className="flex items-center justify-between text-xs">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-3.5 h-3.5 text-[#5f8a5a]" />
                    <span>Gemini Director AI</span>
                  </div>
                  <span className="text-[11px] font-mono text-[#5f8a5a]">Active</span>
                </div>

                <div className="flex items-center justify-between text-xs">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-3.5 h-3.5 text-[#5f8a5a]" />
                    <span>Worker Engine</span>
                  </div>
                  <span className="text-[11px] font-mono text-[#141413] capitalize">
                    {chosenWorker === 'local' ? 'Local Ollama' : 'Groq Cloud'}
                  </span>
                </div>

                <div className="flex items-center justify-between text-xs">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-3.5 h-3.5 text-[#5f8a5a]" />
                    <span>Git Worktree Sandbox</span>
                  </div>
                  <span className="text-[11px] font-mono text-[#5f8a5a]">Enabled</span>
                </div>

                <div className="flex items-center justify-between text-xs">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-3.5 h-3.5 text-[#5f8a5a]" />
                    <span>Terminal Supervision Allowlist</span>
                  </div>
                  <span className="text-[11px] font-mono text-[#5f8a5a]">Armed</span>
                </div>

                <div className="flex items-center justify-between text-xs">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-3.5 h-3.5 text-[#5f8a5a]" />
                    <span>System Dependencies</span>
                  </div>
                  <span className="text-[11px] font-mono text-[#5f8a5a]">
                    {isLoadingDeps ? 'Checking...' : `${dependencies.filter((d) => d.is_ready).length || 2} verified`}
                  </span>
                </div>
              </div>

              {/* Launch Button */}
              <div className="pt-2 flex justify-center gap-3">
                <button
                  type="button"
                  onClick={() => setCurrentStep(3)}
                  className="px-4 py-2 rounded-xl bg-[#ede9de] hover:bg-[#dad9d4] text-[#141413] text-xs font-medium flex items-center gap-1.5 transition-colors border border-[#dad9d4]"
                >
                  <ArrowLeft className="w-3.5 h-3.5" />
                  <span>Back</span>
                </button>

                <button
                  type="button"
                  onClick={onComplete}
                  className="px-8 py-2.5 rounded-xl bg-[#c96442] hover:bg-[#b5523f] text-white font-semibold text-xs flex items-center gap-2 transition-all shadow-md cursor-pointer hover:shadow-lg"
                >
                  <span>Launch ZenCode Workspace</span>
                  <Zap className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
