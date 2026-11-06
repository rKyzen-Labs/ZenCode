import { useState, useRef, useEffect, type FC, type FormEvent, type KeyboardEvent } from 'react';
import {
  Plus,
  ArrowUp,
  ChevronDown,
  Hammer,
  Brain,
  Cpu,
  Sparkles,
} from 'lucide-react';

interface PromptDockProps {
  onSubmit: (prompt: string, mode: 'build' | 'plan') => void;
  isLoading: boolean;
  activeModelName: string;
  isPinned: boolean;
  onSelectModel?: (modelId: string | null) => void;
  placeholder?: string;
}

export const PromptDock: FC<PromptDockProps> = ({
  onSubmit,
  isLoading,
  activeModelName,
  isPinned,
  onSelectModel,
  placeholder = 'Ask anything, / for commands, @ for context...',
}) => {
  const [prompt, setPrompt] = useState('');
  const [mode, setMode] = useState<'build' | 'plan'>('plan');
  const [showModelMenu, setShowModelMenu] = useState(false);
  const [showModeMenu, setShowModeMenu] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 180)}px`;
    }
  }, [prompt]);

  // Click outside to close menus
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setShowModelMenu(false);
        setShowModeMenu(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleSubmit = (e?: FormEvent) => {
    if (e) e.preventDefault();
    if (prompt.trim() && !isLoading) {
      onSubmit(prompt.trim(), mode);
      setPrompt('');
      if (textareaRef.current) {
        textareaRef.current.style.height = 'auto';
      }
    }
  };

  return (
    <div className="w-full max-w-4xl mx-auto px-4 pb-4 pt-2 relative z-20" ref={menuRef}>
      <div className="bg-[#18181b] border border-[#27272a] rounded-2xl shadow-2xl p-3 flex flex-col gap-2.5 transition-all focus-within:border-zinc-600 focus-within:ring-1 focus-within:ring-zinc-600/50">
        {/* Input Textarea */}
        <textarea
          ref={textareaRef}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          rows={1}
          disabled={isLoading}
          className="w-full bg-transparent text-sm text-zinc-100 placeholder:text-zinc-500 focus:outline-none resize-none px-2 py-1 leading-relaxed max-h-44 font-sans"
        />

        {/* Bottom Bar: Attach (+), Mode (Build/Plan), Model Selector, Send Button (matches Example Image 2) */}
        <div className="flex items-center justify-between pt-1 border-t border-[#27272a]/70">
          <div className="flex items-center gap-2">
            {/* Context attach button (+) */}
            <button
              type="button"
              className="w-7 h-7 rounded-lg hover:bg-zinc-800 text-zinc-400 hover:text-zinc-200 flex items-center justify-center transition-colors"
              title="Add Context / Files"
            >
              <Plus className="w-4 h-4" />
            </button>

            {/* Mode Selector (Build / Plan) */}
            <div className="relative">
              <button
                type="button"
                onClick={() => {
                  setShowModeMenu(!showModeMenu);
                  setShowModelMenu(false);
                }}
                className="px-2.5 py-1 rounded-lg hover:bg-zinc-800 text-xs text-zinc-300 font-medium flex items-center gap-1.5 transition-colors"
              >
                {mode === 'build' ? (
                  <>
                    <Hammer className="w-3.5 h-3.5 text-emerald-400" />
                    <span>Build</span>
                  </>
                ) : (
                  <>
                    <Brain className="w-3.5 h-3.5 text-purple-400" />
                    <span>Plan</span>
                  </>
                )}
                <ChevronDown className="w-3 h-3 text-zinc-500" />
              </button>

              {showModeMenu && (
                <div className="absolute bottom-full mb-1 left-0 w-36 bg-[#1f1f23] border border-zinc-700 rounded-xl shadow-xl p-1 text-xs space-y-0.5 z-30">
                  <button
                    type="button"
                    onClick={() => {
                      setMode('plan');
                      setShowModeMenu(false);
                    }}
                    className={`w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-left transition-colors ${
                      mode === 'plan' ? 'bg-purple-500/20 text-purple-200 font-medium' : 'text-zinc-300 hover:bg-zinc-800'
                    }`}
                  >
                    <Brain className="w-3.5 h-3.5 text-purple-400" />
                    <span>Plan (Gemini)</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setMode('build');
                      setShowModeMenu(false);
                    }}
                    className={`w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-left transition-colors ${
                      mode === 'build' ? 'bg-emerald-500/20 text-emerald-200 font-medium' : 'text-zinc-300 hover:bg-zinc-800'
                    }`}
                  >
                    <Hammer className="w-3.5 h-3.5 text-emerald-400" />
                    <span>Build (Local)</span>
                  </button>
                </div>
              )}
            </div>

            {/* Model Selector Pill (matches Example Image 2: "Ollama / Granite 4.1 8B v" or Gemini) */}
            <div className="relative">
              <button
                type="button"
                onClick={() => {
                  setShowModelMenu(!showModelMenu);
                  setShowModeMenu(false);
                }}
                className="px-2.5 py-1 rounded-lg hover:bg-zinc-800 text-xs text-zinc-300 font-mono flex items-center gap-1.5 transition-colors"
              >
                <span className="text-zinc-500">
                  {mode === 'build' ? 'Ollama' : 'Planner'}:
                </span>
                <span className="text-zinc-200">
                  {mode === 'build' ? 'Qwen 3.5 (9B)' : activeModelName}
                </span>
                <ChevronDown className="w-3 h-3 text-zinc-500" />
              </button>

              {showModelMenu && (
                <div className="absolute bottom-full mb-1 left-0 w-64 bg-[#1f1f23] border border-zinc-700 rounded-xl shadow-xl p-1.5 text-xs space-y-1 z-30 font-sans">
                  <div className="text-[10px] uppercase font-semibold text-zinc-500 px-2 py-0.5">
                    Planner Cascade (Google AI Studio)
                  </div>
                  <button
                    type="button"
                    onClick={() => {
                      onSelectModel?.(null);
                      setShowModelMenu(false);
                    }}
                    className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-left transition-colors ${
                      !isPinned ? 'bg-blue-600/20 text-blue-200 font-medium' : 'text-zinc-300 hover:bg-zinc-800'
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <Sparkles className="w-3.5 h-3.5 text-blue-400" />
                      <span>Auto Cascade (3.5 Lite)</span>
                    </div>
                    <span className="text-[9px] px-1 rounded bg-zinc-800 text-zinc-400 font-mono">Free</span>
                  </button>

                  <button
                    type="button"
                    onClick={() => {
                      onSelectModel?.('gemini-3.8-flash');
                      setShowModelMenu(false);
                    }}
                    className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-left transition-colors ${
                      isPinned ? 'bg-purple-600/20 text-purple-200 font-medium' : 'text-zinc-300 hover:bg-zinc-800'
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <Sparkles className="w-3.5 h-3.5 text-purple-400" />
                      <span>Pin: Gemini 3.8 Flash</span>
                    </div>
                    <span className="text-[9px] px-1 rounded bg-purple-900/40 text-purple-300 font-mono">Advanced</span>
                  </button>

                  <div className="text-[10px] uppercase font-semibold text-zinc-500 px-2 pt-1.5 py-0.5 border-t border-zinc-800">
                    Local Worker (Ollama)
                  </div>
                  <div className="px-2.5 py-1 text-zinc-400 text-[11px] flex items-center gap-2">
                    <Cpu className="w-3.5 h-3.5 text-emerald-400" />
                    <span>Qwen 3.5 (9B) — 100% Local Inference</span>
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* Send Button (circle with up arrow) */}
          <button
            type="button"
            onClick={() => handleSubmit()}
            disabled={!prompt.trim() || isLoading}
            className={`w-8 h-8 rounded-full flex items-center justify-center transition-all ${
              prompt.trim() && !isLoading
                ? 'bg-blue-600 hover:bg-blue-500 text-white shadow-md cursor-pointer'
                : 'bg-zinc-800 text-zinc-500 cursor-not-allowed'
            }`}
            title="Send Message (Enter)"
          >
            <ArrowUp className="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>
  );
};
