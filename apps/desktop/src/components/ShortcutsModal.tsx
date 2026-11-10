import React, { useState } from 'react';
import { X, Keyboard, Search, Settings } from 'lucide-react';
import {
  DEFAULT_KEYBINDINGS,
  loadKeybindings,
  formatKeyDisplay,
  type KeybindingDef,
} from '../lib/keybindings';

interface ShortcutsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenSettings: () => void;
}

export const ShortcutsModal: React.FC<ShortcutsModalProps> = ({
  isOpen,
  onClose,
  onOpenSettings,
}) => {
  const [searchTerm, setSearchTerm] = useState('');
  if (!isOpen) return null;

  const currentBindings = loadKeybindings();

  const filtered = DEFAULT_KEYBINDINGS.filter(
    (k) =>
      k.label.toLowerCase().includes(searchTerm.toLowerCase()) ||
      k.description.toLowerCase().includes(searchTerm.toLowerCase()) ||
      (currentBindings[k.id] || k.defaultCombo).toLowerCase().includes(searchTerm.toLowerCase())
  );

  // Group by category
  const categories: Record<string, KeybindingDef[]> = {};
  for (const item of filtered) {
    if (!categories[item.category]) {
      categories[item.category] = [];
    }
    categories[item.category].push(item);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm animate-fade-in">
      <div
        className="w-full max-w-lg bg-[#faf9f5] border border-[#dad9d4] rounded-2xl shadow-2xl flex flex-col overflow-hidden text-[#141413] font-sans animate-scale-up max-h-[85vh]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="px-5 py-4 border-b border-[#dad9d4] bg-[#f7f6f2] flex items-center justify-between shrink-0">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-[#ede9de] border border-[#dad9d4] flex items-center justify-center text-[#c96442]">
              <Keyboard className="w-4 h-4" />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-[#141413]">Keyboard Shortcuts</h3>
              <p className="text-[11px] text-[#83827d]">
                Speed up your workflow with global keyboard commands
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="w-7 h-7 flex items-center justify-center rounded-lg text-[#83827d] hover:text-[#141413] hover:bg-[#ede9de] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Search Bar */}
        <div className="px-5 py-3 border-b border-[#dad9d4]/60 bg-[#faf9f5] shrink-0">
          <div className="relative flex items-center">
            <Search className="w-3.5 h-3.5 text-[#83827d] absolute left-3 pointer-events-none" />
            <input
              type="text"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              placeholder="Filter shortcuts (e.g. tab, search, terminal)..."
              className="w-full pl-9 pr-3 py-1.5 bg-[#ede9de]/50 border border-[#dad9d4] rounded-lg text-xs placeholder:text-[#83827d] focus:outline-none focus:border-[#c96442] focus:bg-[#faf9f5] transition-colors"
              autoFocus
            />
          </div>
        </div>

        {/* Shortcuts list */}
        <div className="p-5 overflow-y-auto space-y-5 flex-1">
          {Object.keys(categories).length === 0 ? (
            <div className="py-8 text-center text-xs text-[#83827d]">
              No shortcuts found matching "{searchTerm}"
            </div>
          ) : (
            Object.entries(categories).map(([category, items]) => (
              <div key={category} className="space-y-2">
                <div className="text-[10.5px] font-semibold uppercase tracking-wider text-[#83827d]">
                  {category}
                </div>
                <div className="bg-[#f7f6f2] border border-[#dad9d4]/80 rounded-xl divide-y divide-[#dad9d4]/50 overflow-hidden">
                  {items.map((item) => {
                    const activeCombo = currentBindings[item.id] || item.defaultCombo;
                    const keys = formatKeyDisplay(activeCombo);
                    return (
                      <div
                        key={item.id}
                        className="flex items-center justify-between px-3.5 py-2.5 hover:bg-[#ede9de]/40 transition-colors"
                      >
                        <div className="pr-3 min-w-0">
                          <div className="text-xs font-medium text-[#141413] truncate">
                            {item.label}
                          </div>
                          <div className="text-[11px] text-[#83827d] truncate">
                            {item.description}
                          </div>
                        </div>

                        {/* Keys Pills */}
                        <div className="flex items-center gap-1 shrink-0">
                          {keys.map((k, i) => (
                            <React.Fragment key={i}>
                              <kbd className="min-w-[22px] px-2 py-0.5 rounded bg-[#faf9f5] border border-[#dad9d4] shadow-sm text-[11px] font-mono font-medium text-[#141413] text-center inline-block">
                                {k}
                              </kbd>
                              {i < keys.length - 1 && (
                                <span className="text-[#83827d] text-[10px]">+</span>
                              )}
                            </React.Fragment>
                          ))}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="px-5 py-3 border-t border-[#dad9d4] bg-[#f7f6f2] flex items-center justify-between shrink-0">
          <span className="text-[11px] text-[#83827d]">
            Tip: Press <kbd className="px-1.5 py-0.5 rounded bg-[#faf9f5] border border-[#dad9d4] text-[10px] font-mono">Esc</kbd> to close
          </span>
          <button
            type="button"
            onClick={() => {
              onClose();
              onOpenSettings();
            }}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-[#ede9de] hover:bg-[#dad9d4] text-[#141413] text-xs font-medium rounded-lg transition-colors border border-[#dad9d4]"
          >
            <Settings className="w-3.5 h-3.5 text-[#83827d]" />
            <span>Customize in Settings</span>
          </button>
        </div>
      </div>
    </div>
  );
};
