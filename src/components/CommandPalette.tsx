import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Archive,
  Clock,
  Cpu,
  FileEdit,
  Gauge,
  Inbox,
  Lock,
  PenSquare,
  Plus,
  RefreshCw,
  Search,
  Send,
  Settings,
  ShieldAlert,
  Trash2,
} from 'lucide-react';
import { formatRelativeTime } from '../lib/format';
import { searchMessages } from '../lib/tauri';
import type { Account, FolderRole, SearchResponse } from '../types/mail';

interface CommandPaletteProps {
  open: boolean;
  accounts: Account[];
  activeAccountId: string | null;
  selectedCount: number;
  benchmark50k: boolean;
  onClose: () => void;
  onSelectSearchResult: (threadId: string, folderRole: FolderRole) => void;
  onSelectAccount: (accountId: string | null) => void;
  onJumpFolder: (folder: FolderRole) => void;
  onCompose: () => void;
  onOpenAccountSetup: () => void;
  onOpenSettings: () => void;
  onSyncAll: () => void;
  onBulkArchive: () => void;
  onToggleBenchmark50k: () => void;
}

interface CommandItem {
  id: string;
  section: string;
  title: string;
  subtitle?: string;
  shortcut?: string;
  icon: typeof Inbox;
  run: () => void;
}

export function CommandPalette({
  open,
  accounts,
  activeAccountId,
  selectedCount,
  benchmark50k,
  onClose,
  onSelectSearchResult,
  onSelectAccount,
  onJumpFolder,
  onCompose,
  onOpenAccountSetup,
  onOpenSettings,
  onSyncAll,
  onBulkArchive,
  onToggleBenchmark50k,
}: CommandPaletteProps) {
  const [query, setQuery] = useState('');
  const [ftsResponse, setFtsResponse] = useState<SearchResponse | null>(null);
  const [activeIdx, setActiveIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (open) {
      setQuery('');
      setFtsResponse(null);
      setActiveIdx(0);
      setTimeout(() => inputRef.current?.focus(), 10);
    }
  }, [open]);

  // Execute sub-20ms local SQLite FTS5 query whenever input changes
  useEffect(() => {
    if (!open) return;
    const trimmed = query.trim();
    if (trimmed.length < 2) {
      setFtsResponse(null);
      return;
    }
    let cancelled = false;
    searchMessages(trimmed, activeAccountId).then((res) => {
      if (!cancelled) {
        setFtsResponse(res);
        setActiveIdx(0);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [open, query, activeAccountId]);

  const commands = useMemo<CommandItem[]>(() => {
    const items: CommandItem[] = [
      {
        id: 'cmd-compose',
        section: 'Actions',
        title: 'Compose New Encrypted Email',
        subtitle: 'Direct SMTP delivery with optional PGP signing',
        shortcut: 'C',
        icon: PenSquare,
        run: () => {
          onClose();
          onCompose();
        },
      },
      {
        id: 'cmd-add-account',
        section: 'Actions',
        title: 'Add Email Account (Gmail / iCloud / Proton Bridge / IMAP)',
        subtitle: 'Run live TLS 1.3 handshake & store secret in macOS Keychain',
        icon: Plus,
        run: () => {
          onClose();
          onOpenAccountSetup();
        },
      },
      {
        id: 'cmd-sync-all',
        section: 'Actions',
        title: 'Sync All Accounts Now',
        subtitle: 'Two-phase IMAP header-first fetch + body sanitization',
        icon: RefreshCw,
        run: () => {
          onClose();
          onSyncAll();
        },
      },
      {
        id: 'cmd-settings',
        section: 'Actions',
        title: 'Local AI (Ollama / MLX) & Vault Settings',
        subtitle: 'Configure localhost:11434 inference and privacy shield',
        icon: Cpu,
        run: () => {
          onClose();
          onOpenSettings();
        },
      },
      {
        id: 'cmd-benchmark',
        section: 'Actions',
        title: benchmark50k
          ? 'Disable 50,000-Row Virtualizer Benchmark'
          : 'Run 50,000-Row 120fps Virtualizer Benchmark',
        subtitle: 'Mount 50,000 threads to verify zero layout shift scrolling',
        icon: Gauge,
        run: () => {
          onClose();
          onToggleBenchmark50k();
        },
      },
    ];

    if (selectedCount > 0) {
      items.unshift({
        id: 'cmd-bulk-archive',
        section: 'Bulk Actions',
        title: `Archive ${selectedCount} Selected Threads`,
        shortcut: 'E',
        icon: Archive,
        run: () => {
          onClose();
          onBulkArchive();
        },
      });
    }

    // Folder jump commands
    const folders: Array<[FolderRole, string, string, typeof Inbox]> = [
      ['inbox', 'Jump to Unified Inbox', 'G I', Inbox],
      ['snoozed', 'Jump to Snoozed', 'G Z', Clock],
      ['sent', 'Jump to Sent', 'G S', Send],
      ['drafts', 'Jump to Drafts', 'G D', FileEdit],
      ['archive', 'Jump to Vault / Archive', 'G V', Archive],
      ['trash', 'Jump to Trash', 'G T', Trash2],
    ];
    for (const [role, title, shortcut, icon] of folders) {
      items.push({
        id: `folder-${role}`,
        section: 'Navigate Folders',
        title,
        shortcut,
        icon,
        run: () => {
          onJumpFolder(role);
          onClose();
        },
      });
    }

    // Account switching commands
    items.push({
      id: 'acc-all',
      section: 'Switch Account',
      title: 'All Accounts (Unified View)',
      subtitle: `${accounts.length} direct IMAP accounts`,
      icon: Settings,
      run: () => {
        onSelectAccount(null);
        onClose();
      },
    });
    for (const acc of accounts) {
      items.push({
        id: `acc-${acc.id}`,
        section: 'Switch Account',
        title: `Switch to ${acc.email}`,
        subtitle: `${acc.imapHost}:${acc.imapPort} · ${acc.provider}`,
        icon: Lock,
        run: () => {
          onSelectAccount(acc.id);
          onClose();
        },
      });
    }

    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter(
      (i) =>
        i.title.toLowerCase().includes(q) ||
        (i.subtitle && i.subtitle.toLowerCase().includes(q)) ||
        i.section.toLowerCase().includes(q),
    );
  }, [
    accounts,
    benchmark50k,
    onBulkArchive,
    onClose,
    onCompose,
    onJumpFolder,
    onOpenAccountSetup,
    onOpenSettings,
    onSelectAccount,
    onSyncAll,
    onToggleBenchmark50k,
    query,
    selectedCount,
  ]);

  const ftsItems = ftsResponse?.results ?? [];
  const totalSelectable = ftsItems.length + commands.length;

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setActiveIdx((idx) => (totalSelectable === 0 ? 0 : (idx + 1) % totalSelectable));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setActiveIdx((idx) =>
          totalSelectable === 0 ? 0 : (idx - 1 + totalSelectable) % totalSelectable,
        );
      } else if (e.key === 'Enter') {
        e.preventDefault();
        if (activeIdx < ftsItems.length) {
          const hit = ftsItems[activeIdx];
          onSelectSearchResult(hit.threadId, (hit.folderRole as FolderRole) || 'inbox');
          onClose();
        } else {
          const cmd = commands[activeIdx - ftsItems.length];
          if (cmd) cmd.run();
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, activeIdx, totalSelectable, ftsItems, commands, onSelectSearchResult, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 bg-black/70 backdrop-blur-[2px] flex items-start justify-center pt-[12vh] px-4"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-label="Command Palette and FTS5 Search"
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-[640px] rounded-xl bg-[#11141c] border border-white/[0.12] shadow-2xl overflow-hidden flex flex-col max-h-[70vh]"
      >
        {/* Top Search Input */}
        <div className="h-12 px-4 border-b border-white/[0.07] flex items-center gap-3">
          <Search className="w-4 h-4 text-emerald-400 shrink-0" />
          <input
            ref={inputRef}
            id="input-command-palette"
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search encrypted FTS5 index, jump to folder, switch account, or run command…"
            className="flex-1 bg-transparent border-0 focus:outline-none text-[13.5px] text-[#f1f5f9] placeholder-[#596174]"
          />
          {ftsResponse && (
            <span className="text-[10.5px] font-mono tabular-nums px-1.5 py-0.5 rounded bg-emerald-500/12 text-emerald-300 shrink-0">
              FTS5 · {ftsResponse.queryTimeMs.toFixed(2)}ms
            </span>
          )}
          <span className="kbd">ESC</span>
        </div>

        {/* Scrollable Results */}
        <div className="flex-1 overflow-y-auto p-2 space-y-3">
          {/* FTS5 Local Search Matches */}
          {ftsItems.length > 0 && (
            <div className="space-y-1">
              <div className="px-2.5 py-1 flex items-center justify-between text-[10px] font-mono uppercase tracking-wider text-emerald-400/90">
                <span>Local FTS5 Full-Text Hits ({ftsItems.length})</span>
                <span>BM25 Ranked</span>
              </div>
              {ftsItems.map((hit, idx) => {
                const selected = idx === activeIdx;
                return (
                  <button
                    key={hit.messageId}
                    onMouseEnter={() => setActiveIdx(idx)}
                    onClick={() => {
                      onSelectSearchResult(
                        hit.threadId,
                        (hit.folderRole as FolderRole) || 'inbox',
                      );
                      onClose();
                    }}
                    className={`w-full px-3 py-2 rounded-lg text-left transition-colors ${
                      selected ? 'bg-white/[0.08]' : 'hover:bg-white/[0.04]'
                    }`}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-[12.5px] font-medium text-[#f1f5f9] truncate">
                        {hit.subject}
                      </span>
                      <div className="flex items-center gap-1.5 shrink-0">
                        {hit.pgpStatus !== 'none' && (
                          <Lock className="w-3 h-3 text-emerald-400" />
                        )}
                        {hit.trackersBlocked > 0 && (
                          <span className="text-[10px] font-mono text-amber-300 flex items-center gap-0.5">
                            <ShieldAlert className="w-2.5 h-2.5" />
                            {hit.trackersBlocked}
                          </span>
                        )}
                        <span className="text-[10.5px] font-mono text-[#667085]">
                          {formatRelativeTime(hit.receivedAt)}
                        </span>
                      </div>
                    </div>
                    <div className="text-[11px] text-[#7c8599] flex items-center gap-2 mt-0.5">
                      <span className="font-mono text-[#9ba3b5]">{hit.senderName}</span>
                      <span>·</span>
                      <span
                        className="truncate"
                        dangerouslySetInnerHTML={{ __html: hit.highlightedSnippet }}
                      />
                    </div>
                  </button>
                );
              })}
            </div>
          )}

          {/* Commands & Navigation */}
          {commands.length > 0 && (
            <div className="space-y-1">
              <div className="px-2.5 py-1 text-[10px] font-mono uppercase tracking-wider text-[#596174]">
                Commands &amp; Navigation
              </div>
              {commands.map((cmd, cIdx) => {
                const globalIdx = ftsItems.length + cIdx;
                const selected = globalIdx === activeIdx;
                const Icon = cmd.icon;
                return (
                  <button
                    key={cmd.id}
                    onMouseEnter={() => setActiveIdx(globalIdx)}
                    onClick={cmd.run}
                    className={`w-full px-3 py-2 rounded-lg flex items-center justify-between gap-3 text-left transition-colors ${
                      selected ? 'bg-white/[0.08]' : 'hover:bg-white/[0.04]'
                    }`}
                  >
                    <div className="flex items-center gap-2.5 min-w-0">
                      <Icon className="w-4 h-4 text-[#8d96a8] shrink-0" />
                      <div className="min-w-0">
                        <div className="text-[12.5px] font-medium text-[#edf0f5] truncate">
                          {cmd.title}
                        </div>
                        {cmd.subtitle && (
                          <div className="text-[11px] text-[#667085] truncate">{cmd.subtitle}</div>
                        )}
                      </div>
                    </div>
                    <div className="flex items-center gap-2 shrink-0">
                      <span className="text-[10px] font-mono text-[#596174]">{cmd.section}</span>
                      {cmd.shortcut && <span className="kbd">{cmd.shortcut}</span>}
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
