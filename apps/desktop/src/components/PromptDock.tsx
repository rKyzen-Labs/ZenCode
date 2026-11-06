import { useState, useRef, useEffect, type FC, type FormEvent, type KeyboardEvent } from 'react';

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
  placeholder = 'ask anything, / for commands, @ for context...',
}) => {
  const [prompt, setPrompt] = useState('');
  const [mode, setMode] = useState<'build' | 'plan'>('plan');
  const [showModelMenu, setShowModelMenu] = useState(false);
  const [showModeMenu, setShowModeMenu] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 160)}px`;
    }
  }, [prompt]);

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
    <div className="w-full max-w-4xl mx-auto px-4 pb-3 pt-1 relative z-20 font-mono" ref={menuRef}>
      <div className="bg-[#121214] border border-[#27272a] rounded-lg p-2.5 flex flex-col gap-2 shadow-lg focus-within:border-zinc-500 transition-colors">
        {/* Textarea */}
        <textarea
          ref={textareaRef}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          rows={1}
          disabled={isLoading}
          className="w-full bg-transparent text-xs text-zinc-100 placeholder:text-zinc-600 focus:outline-none resize-none px-1 py-0.5 leading-relaxed max-h-40 font-mono"
        />

        {/* Monochromatic Bottom Bar */}
        <div className="flex items-center justify-between pt-1 border-t border-[#1f1f23] text-xs">
          <div className="flex items-center gap-1.5">
            {/* Monospaced context button [+] */}
            <button
              type="button"
              className="px-1.5 py-0.5 text-zinc-500 hover:text-zinc-200 hover:bg-[#1f1f23] rounded transition-colors text-[11px]"
              title="add context"
            >
              [+]
            </button>

            {/* Mode Selector */}
            <div className="relative">
              <button
                type="button"
                onClick={() => {
                  setShowModeMenu(!showModeMenu);
                  setShowModelMenu(false);
                }}
                className="px-2 py-0.5 text-[11px] text-zinc-400 hover:text-zinc-200 hover:bg-[#1f1f23] rounded transition-colors"
              >
                [mode: {mode} v]
              </button>

              {showModeMenu && (
                <div className="absolute bottom-full mb-1 left-0 w-36 bg-[#18181b] border border-[#27272a] rounded shadow-xl p-1 text-[11px] space-y-0.5 z-30">
                  <button
                    type="button"
                    onClick={() => {
                      setMode('plan');
                      setShowModeMenu(false);
                    }}
                    className={`w-full text-left px-2 py-1 rounded transition-colors ${
                      mode === 'plan' ? 'bg-[#27272a] text-white' : 'text-zinc-400 hover:text-zinc-200 hover:bg-[#1f1f23]'
                    }`}
                  >
                    plan (remote)
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setMode('build');
                      setShowModeMenu(false);
                    }}
                    className={`w-full text-left px-2 py-1 rounded transition-colors ${
                      mode === 'build' ? 'bg-[#27272a] text-white' : 'text-zinc-400 hover:text-zinc-200 hover:bg-[#1f1f23]'
                    }`}
                  >
                    build (local)
                  </button>
                </div>
              )}
            </div>

            {/* Model Selector */}
            <div className="relative">
              <button
                type="button"
                onClick={() => {
                  setShowModelMenu(!showModelMenu);
                  setShowModeMenu(false);
                }}
                className="px-2 py-0.5 text-[11px] text-zinc-400 hover:text-zinc-200 hover:bg-[#1f1f23] rounded transition-colors"
              >
                [model: {mode === 'build' ? 'qwen3.5:9b' : activeModelName} v]
              </button>

              {showModelMenu && (
                <div className="absolute bottom-full mb-1 left-0 w-60 bg-[#18181b] border border-[#27272a] rounded shadow-xl p-1 text-[11px] space-y-0.5 z-30">
                  <div className="text-[10px] text-zinc-600 px-2 py-0.5 uppercase tracking-wider">
                    planner (ai studio)
                  </div>
                  <button
                    type="button"
                    onClick={() => {
                      onSelectModel?.(null);
                      setShowModelMenu(false);
                    }}
                    className={`w-full text-left px-2 py-1 rounded transition-colors ${
                      !isPinned ? 'bg-[#27272a] text-white' : 'text-zinc-400 hover:text-zinc-200 hover:bg-[#1f1f23]'
                    }`}
                  >
                    auto-cascade (3.5 lite)
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      onSelectModel?.('gemini-3.8-flash');
                      setShowModelMenu(false);
                    }}
                    className={`w-full text-left px-2 py-1 rounded transition-colors ${
                      isPinned ? 'bg-[#27272a] text-white' : 'text-zinc-400 hover:text-zinc-200 hover:bg-[#1f1f23]'
                    }`}
                  >
                    pin: gemini-3.8-flash
                  </button>

                  <div className="text-[10px] text-zinc-600 px-2 pt-1 uppercase tracking-wider border-t border-[#27272a]">
                    worker (ollama local)
                  </div>
                  <div className="px-2 py-1 text-zinc-500 text-[10px]">
                    qwen3.5:9b [offline inference]
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* Minimalist Monospaced Send Button */}
          <button
            type="button"
            onClick={() => handleSubmit()}
            disabled={!prompt.trim() || isLoading}
            className={`px-3 py-1 rounded text-xs font-mono transition-colors ${
              prompt.trim() && !isLoading
                ? 'bg-zinc-200 text-black hover:bg-white font-medium cursor-pointer'
                : 'bg-[#1f1f23] text-zinc-600 cursor-not-allowed'
            }`}
          >
            [send ^]
          </button>
        </div>
      </div>
    </div>
  );
};
