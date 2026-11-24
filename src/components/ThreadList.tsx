import { useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import {
  Archive,
  CheckSquare,
  Gauge,
  Lock,
  Paperclip,
  Search,
  ShieldAlert,
  ShieldCheck,
  Square,
  Star,
  Trash2,
  X,
} from 'lucide-react';
import { formatRelativeTime } from '../lib/format';
import type { Account, FolderRole, ThreadSummary } from '../types/mail';

type QuickFilter = 'all' | 'unread' | 'pgp' | 'trackers' | 'attachments';

interface ThreadListProps {
  threads: ThreadSummary[];
  accounts: Account[];
  activeFolder: FolderRole;
  activeThreadId: string | null;
  selectedIds: Set<string>;
  searchQuery: string;
  searchLatencyMs: number | null;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
  benchmark50k: boolean;
  onToggleBenchmark50k: () => void;
  onSearchChange: (value: string) => void;
  onSelectThread: (threadId: string) => void;
  onToggleCheckbox: (threadId: string) => void;
  onClearSelection: () => void;
  onBulkArchive: () => void;
  onBulkTrash: () => void;
  onBulkMarkRead: () => void;
  onToggleStar: (threadId: string) => void;
}

const FOLDER_TITLES: Record<FolderRole, string> = {
  inbox: 'Inbox',
  snoozed: 'Snoozed',
  sent: 'Sent',
  drafts: 'Drafts',
  archive: 'Vault / Archive',
  trash: 'Trash',
};

const ROW_HEIGHT = 76;

export function ThreadList({
  threads,
  accounts,
  activeFolder,
  activeThreadId,
  selectedIds,
  searchQuery,
  searchLatencyMs,
  searchInputRef,
  benchmark50k,
  onToggleBenchmark50k,
  onSearchChange,
  onSelectThread,
  onToggleCheckbox,
  onClearSelection,
  onBulkArchive,
  onBulkTrash,
  onBulkMarkRead,
  onToggleStar,
}: ThreadListProps) {
  const [quickFilter, setQuickFilter] = useState<QuickFilter>('all');
  const parentRef = useRef<HTMLDivElement | null>(null);

  const accountColorMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const acc of accounts) {
      map.set(acc.id, acc.accentColor);
    }
    return map;
  }, [accounts]);

  const filteredThreads = useMemo(() => {
    switch (quickFilter) {
      case 'unread':
        return threads.filter((t) => t.unreadCount > 0);
      case 'pgp':
        return threads.filter((t) => t.pgpStatus !== 'none');
      case 'trackers':
        return threads.filter((t) => t.trackersBlocked > 0);
      case 'attachments':
        return threads.filter((t) => t.hasAttachments);
      default:
        return threads;
    }
  }, [threads, quickFilter]);

  const rowVirtualizer = useVirtualizer({
    count: filteredThreads.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 14,
  });

  // Keep active keyboard cursor smoothly scrolled into view during j/k navigation
  useEffect(() => {
    if (!activeThreadId) return;
    const idx = filteredThreads.findIndex((t) => t.id === activeThreadId);
    if (idx >= 0) {
      rowVirtualizer.scrollToIndex(idx, { align: 'auto' });
    }
  }, [activeThreadId, filteredThreads, rowVirtualizer]);

  return (
    <section
      aria-label="Thread List"
      className="w-[376px] xl:w-[404px] h-full flex flex-col bg-[#0b0c10] border-r border-white/[0.068] shrink-0"
    >
      {/* Top Folder Header + 50k Virtualizer Benchmark Switch */}
      <div
        data-tauri-drag-region
        className="titlebar-drag h-12 px-3.5 flex items-center justify-between border-b border-white/[0.055]"
      >
        <div data-tauri-drag-region className="flex items-center gap-2">
          <h1
            data-tauri-drag-region
            className="text-[13.5px] font-semibold tracking-[-0.015em] text-[#f1f5f9]"
          >
            {FOLDER_TITLES[activeFolder]}
          </h1>
          <span
            data-tauri-drag-region
            className="text-[11px] font-mono tabular-nums text-[#667085]"
          >
            {filteredThreads.length.toLocaleString()}
          </span>
        </div>

        <button
          id="btn-benchmark-50k"
          onClick={onToggleBenchmark50k}
          title="Mount 50,000 rows to stress-test 120fps virtualized scrolling"
          className={`titlebar-no-drag h-6 px-2 rounded text-[10.5px] font-mono flex items-center gap-1.5 border transition-colors ${
            benchmark50k
              ? 'bg-emerald-500/15 border-emerald-500/30 text-emerald-300'
              : 'bg-white/[0.03] border-white/[0.07] text-[#7c8599] hover:text-[#dce2ec] hover:bg-white/[0.06]'
          }`}
        >
          <Gauge className="w-3 h-3" />
          <span>{benchmark50k ? '50k Live' : '50k Test'}</span>
        </button>
      </div>

      {/* Inline Instant FTS5 Search Input */}
      <div className="px-3 pt-2.5 pb-2 border-b border-white/[0.055] space-y-2">
        <div className="relative flex items-center">
          <Search className="w-3.5 h-3.5 text-[#596174] absolute left-2.5 pointer-events-none" />
          <input
            ref={searchInputRef}
            id="input-fts5-search"
            type="text"
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
            placeholder="Instant FTS5 search... (press /)"
            className="w-full h-8 pl-8 pr-16 rounded-md bg-[#11141b] border border-white/[0.07] focus:border-white/[0.18] focus:outline-none text-[12.5px] text-[#edf0f5] placeholder-[#596174] transition-colors"
          />
          <div className="absolute right-2 flex items-center gap-1.5">
            {searchQuery && searchLatencyMs !== null && (
              <span className="text-[10px] font-mono tabular-nums text-emerald-400">
                {searchLatencyMs.toFixed(1)}ms
              </span>
            )}
            {searchQuery ? (
              <button
                onClick={() => onSearchChange('')}
                className="p-0.5 rounded text-[#667085] hover:text-[#edf0f5]"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            ) : (
              <span className="kbd">/</span>
            )}
          </div>
        </div>

        {/* Tactical Filter Pills OR Bulk Action Bar when rows are selected (`x`) */}
        {selectedIds.size > 0 ? (
          <div className="h-7 px-2 rounded-md bg-[#161b26] border border-white/[0.1] flex items-center justify-between text-[11.5px]">
            <span className="font-mono text-[#edf0f5] tabular-nums">
              {selectedIds.size} selected
            </span>
            <div className="flex items-center gap-1">
              <button
                onClick={onBulkArchive}
                className="px-2 py-0.5 rounded hover:bg-white/[0.08] text-[#cbd5e1] flex items-center gap-1"
                title="Archive selected (e)"
              >
                <Archive className="w-3 h-3" />
                <span>Archive</span>
              </button>
              <button
                onClick={onBulkMarkRead}
                className="px-2 py-0.5 rounded hover:bg-white/[0.08] text-[#cbd5e1]"
                title="Mark read (u)"
              >
                Read
              </button>
              <button
                onClick={onBulkTrash}
                className="px-2 py-0.5 rounded hover:bg-rose-500/20 text-rose-300 flex items-center gap-1"
                title="Move to trash (#)"
              >
                <Trash2 className="w-3 h-3" />
              </button>
              <button
                onClick={onClearSelection}
                className="p-0.5 rounded text-[#7c8599] hover:text-white"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        ) : (
          <div className="flex items-center gap-1 overflow-x-auto no-scrollbar">
            {(
              [
                ['all', 'All'],
                ['unread', 'Unread'],
                ['pgp', 'PGP'],
                ['trackers', 'Shielded'],
                ['attachments', 'Files'],
              ] as const
            ).map(([key, label]) => (
              <button
                key={key}
                onClick={() => setQuickFilter(key)}
                className={`h-6 px-2 rounded text-[11px] font-medium transition-colors shrink-0 ${
                  quickFilter === key
                    ? 'bg-white/[0.1] text-[#f3f6fb]'
                    : 'text-[#667085] hover:text-[#b4bdd0] hover:bg-white/[0.04]'
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* 120fps Virtualized Scroll Viewport */}
      <div
        ref={parentRef}
        className="flex-1 overflow-y-auto relative"
        role="listbox"
        aria-label="Email threads"
      >
        {filteredThreads.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center px-6 text-center">
            <div className="w-10 h-10 rounded-lg bg-white/[0.03] border border-white/[0.07] flex items-center justify-center mb-3">
              <ShieldCheck className="w-5 h-5 text-emerald-400/70" />
            </div>
            <p className="text-[13px] font-medium text-[#cbd5e1]">
              {searchQuery ? 'Zero local FTS5 matches' : 'Inbox Zero'}
            </p>
            <p className="text-[11.5px] text-[#596174] mt-1">
              {searchQuery
                ? `No encrypted local messages matched "${searchQuery}".`
                : 'All clear. Press C to compose or ⌘K to jump folders.'}
            </p>
          </div>
        ) : (
          <div
            style={{
              height: `${rowVirtualizer.getTotalSize()}px`,
              width: '100%',
              position: 'relative',
            }}
          >
            {rowVirtualizer.getVirtualItems().map((virtualRow) => {
              const thread = filteredThreads[virtualRow.index];
              const isActive = thread.id === activeThreadId;
              const isSelected = selectedIds.has(thread.id);
              const isUnread = thread.unreadCount > 0;
              const accColor = accountColorMap.get(thread.accountId) || '#10b981';

              return (
                <div
                  key={thread.id}
                  role="option"
                  aria-selected={isActive}
                  onClick={() => onSelectThread(thread.id)}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    height: `${ROW_HEIGHT}px`,
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                  className={`virtual-thread-row px-3 py-2.5 border-b border-white/[0.045] cursor-pointer flex items-start gap-2.5 transition-colors group ${
                    isActive
                      ? 'bg-[#161a24] shadow-[inset_2px_0_0_0_#e2e8f0]'
                      : isSelected
                        ? 'bg-emerald-500/[0.06] shadow-[inset_2px_0_0_0_#10b981]'
                        : 'hover:bg-white/[0.028]'
                  }`}
                >
                  {/* Multi-select checkbox (toggled via click or `x`) */}
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onToggleCheckbox(thread.id);
                    }}
                    aria-label="Select thread"
                    className={`mt-0.5 shrink-0 transition-opacity ${
                      isSelected
                        ? 'text-emerald-400 opacity-100'
                        : 'text-[#495163] opacity-0 group-hover:opacity-100'
                    }`}
                  >
                    {isSelected ? (
                      <CheckSquare className="w-3.5 h-3.5" />
                    ) : (
                      <Square className="w-3.5 h-3.5" />
                    )}
                  </button>

                  {/* Main Thread Row Metadata */}
                  <div className="min-w-0 flex-1 flex flex-col gap-0.5">
                    {/* Row 1: Account dot + Unread indicator + Sender + Timestamp */}
                    <div className="flex items-center justify-between gap-2">
                      <div className="flex items-center gap-1.5 min-w-0">
                        <span
                          className="w-1.5 h-1.5 rounded-full shrink-0"
                          style={{
                            backgroundColor: isUnread ? accColor : 'rgba(255,255,255,0.14)',
                          }}
                        />
                        <span
                          className={`text-[12.5px] truncate ${
                            isUnread ? 'font-semibold text-[#f5f7fb]' : 'font-normal text-[#9ca5b8]'
                          }`}
                        >
                          {thread.lastSenderName}
                        </span>
                        {thread.messageCount > 1 && (
                          <span className="text-[10px] font-mono text-[#667085] bg-white/[0.05] px-1 rounded">
                            {thread.messageCount}
                          </span>
                        )}
                      </div>

                      <div className="flex items-center gap-1.5 shrink-0">
                        {thread.hasAttachments && (
                          <Paperclip className="w-3 h-3 text-[#596174]" />
                        )}
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            onToggleStar(thread.id);
                          }}
                          className={`transition-opacity ${
                            thread.isStarred
                              ? 'text-amber-400 opacity-100'
                              : 'text-[#495163] opacity-0 group-hover:opacity-100 hover:text-amber-300'
                          }`}
                          title="Star thread (s)"
                        >
                          <Star
                            className="w-3 h-3"
                            fill={thread.isStarred ? 'currentColor' : 'none'}
                          />
                        </button>
                        <span className="text-[11px] font-mono tabular-nums text-[#667085]">
                          {formatRelativeTime(thread.lastMessageAt)}
                        </span>
                      </div>
                    </div>

                    {/* Row 2: Subject + PGP / Tracker Badges */}
                    <div className="flex items-center justify-between gap-2">
                      <span
                        className={`text-[12.5px] truncate ${
                          isUnread ? 'text-[#e2e8f0] font-medium' : 'text-[#8d96a8]'
                        }`}
                      >
                        {thread.subject}
                      </span>

                      <div className="flex items-center gap-1 shrink-0">
                        {thread.pgpStatus !== 'none' && (
                          <span
                            title={`PGP: ${thread.pgpStatus}`}
                            className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded bg-emerald-500/12 border border-emerald-500/20 text-[9.5px] font-mono text-emerald-300"
                          >
                            <Lock className="w-2.5 h-2.5" />
                            {thread.pgpStatus.includes('encrypted') ? 'ENC' : 'PGP'}
                          </span>
                        )}
                        {thread.trackersBlocked > 0 && (
                          <span
                            title={`${thread.trackersBlocked} trackers stripped by Rust sanitizer`}
                            className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded bg-amber-500/10 border border-amber-500/20 text-[9.5px] font-mono tabular-nums text-amber-300"
                          >
                            <ShieldAlert className="w-2.5 h-2.5" />
                            {thread.trackersBlocked}
                          </span>
                        )}
                      </div>
                    </div>

                    {/* Row 3: Single-line Snippet */}
                    <p className="text-[11.5px] text-[#5c6478] truncate m-0 leading-snug">
                      {thread.snippet}
                    </p>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Bottom Keyboard Hint Footer */}
      <div className="h-7 px-3 border-t border-white/[0.055] bg-[#0a0b0f] flex items-center justify-between text-[10.5px] text-[#596174]">
        <div className="flex items-center gap-2.5">
          <span className="flex items-center gap-1">
            <span className="kbd">J</span>
            <span className="kbd">K</span>
            <span>nav</span>
          </span>
          <span className="flex items-center gap-1">
            <span className="kbd">E</span>
            <span>archive</span>
          </span>
          <span className="flex items-center gap-1">
            <span className="kbd">X</span>
            <span>select</span>
          </span>
        </div>
        <span className="flex items-center gap-1">
          <span className="kbd">?</span>
          <span>keys</span>
        </span>
      </div>
    </section>
  );
}
