import { X } from 'lucide-react';

interface ShortcutsModalProps {
  open: boolean;
  onClose: () => void;
}

const GROUPS = [
  {
    title: 'Navigation',
    items: [
      { keys: ['J', '↓'], desc: 'Next thread' },
      { keys: ['K', '↑'], desc: 'Previous thread' },
      { keys: ['⌘K'], desc: 'Command palette & FTS5 search' },
      { keys: ['/'], desc: 'Focus inline FTS5 search' },
      { keys: ['G', 'I'], desc: 'Jump to Unified Inbox' },
      { keys: ['G', 'V'], desc: 'Jump to Vault / Archive' },
      { keys: ['G', 'S'], desc: 'Jump to Sent' },
      { keys: ['G', 'Z'], desc: 'Jump to Snoozed' },
    ],
  },
  {
    title: 'Thread Actions',
    items: [
      { keys: ['E'], desc: 'Archive thread (or selected)' },
      { keys: ['R'], desc: 'Reply to active thread' },
      { keys: ['C'], desc: 'Compose new message' },
      { keys: ['X'], desc: 'Select / toggle multi-select' },
      { keys: ['U'], desc: 'Toggle read / unread' },
      { keys: ['S'], desc: 'Star / unstar thread' },
      { keys: ['H'], desc: 'Snooze until tomorrow' },
      { keys: ['#'], desc: 'Move to Trash' },
    ],
  },
];

export function ShortcutsModal({ open, onClose }: ShortcutsModalProps) {
  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 bg-black/75 backdrop-blur-[2px] flex items-center justify-center p-4"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-label="Keyboard Shortcuts"
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-[560px] rounded-xl bg-[#11141c] border border-white/[0.12] shadow-2xl overflow-hidden"
      >
        <div className="px-5 py-3.5 border-b border-white/[0.07] flex items-center justify-between">
          <h2 className="text-[14px] font-semibold text-[#f1f5f9]">
            Keyboard-First Tactical Navigation
          </h2>
          <button
            onClick={onClose}
            className="p-1 rounded text-[#667085] hover:text-white hover:bg-white/[0.06]"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 grid grid-cols-1 sm:grid-cols-2 gap-6">
          {GROUPS.map((g) => (
            <div key={g.title} className="space-y-2.5">
              <div className="text-[10.5px] font-mono uppercase tracking-wider text-emerald-400">
                {g.title}
              </div>
              <div className="space-y-2">
                {g.items.map((item) => (
                  <div
                    key={item.desc}
                    className="flex items-center justify-between text-[12px] text-[#cbd5e1]"
                  >
                    <span>{item.desc}</span>
                    <div className="flex items-center gap-1">
                      {item.keys.map((k) => (
                        <span key={k} className="kbd">
                          {k}
                        </span>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
