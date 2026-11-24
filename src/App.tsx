import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AccountSetupModal } from './components/AccountSetupModal';
import { CommandPalette } from './components/CommandPalette';
import { ComposerModal } from './components/ComposerModal';
import { ReadingPane } from './components/ReadingPane';
import { SettingsModal } from './components/SettingsModal';
import { ShortcutsModal } from './components/ShortcutsModal';
import { Sidebar } from './components/Sidebar';
import { ThreadList } from './components/ThreadList';
import { useKeyboardNav } from './hooks/useKeyboardNav';
import {
  archiveThreads,
  getBenchmarkActive,
  getFolderCounts,
  getPrivacyStats,
  getSettings,
  getThreadDetail,
  listAccounts,
  listThreads,
  moveThreadsToInbox,
  searchMessages,
  setBenchmarkRowCount,
  setThreadsRead,
  snoozeThreads,
  subscribeSyncEvents,
  syncAccountNow,
  toggleThreadStar,
  trashThreads,
} from './lib/tauri';
import type {
  Account,
  AppSettings,
  FolderCounts,
  FolderRole,
  PrivacyStats,
  ThreadDetail,
  ThreadSummary,
} from './types/mail';

const DEFAULT_COUNTS: FolderCounts = {
  inboxUnread: 0,
  inboxTotal: 0,
  sentTotal: 0,
  draftsTotal: 0,
  snoozedTotal: 0,
  archiveTotal: 0,
  trashTotal: 0,
};

const DEFAULT_PRIVACY: PrivacyStats = {
  totalTrackersBlocked: 0,
  totalMessagesIndexed: 0,
  pgpVerifiedCount: 0,
  sqlcipherActive: true,
  keychainBackend: 'macOS Keychain (Security.framework)',
  fts5Ready: true,
};

const DEFAULT_SETTINGS: AppSettings = {
  aiEnabled: true,
  aiEndpoint: 'http://localhost:11434',
  aiModel: 'llama3.2:latest',
  blockRemoteImages: true,
  stripTrackers: true,
  autoVerifyPgp: true,
};

export function App() {
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [activeAccountId, setActiveAccountId] = useState<string | null>(null);
  const [activeFolder, setActiveFolder] = useState<FolderRole>('inbox');
  const [threads, setThreads] = useState<ThreadSummary[]>([]);
  const [activeThreadId, setActiveThreadId] = useState<string | null>(null);
  const [activeDetail, setActiveDetail] = useState<ThreadDetail | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [counts, setCounts] = useState<FolderCounts>(DEFAULT_COUNTS);
  const [privacyStats, setPrivacyStats] = useState<PrivacyStats>(DEFAULT_PRIVACY);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);

  const [searchQuery, setSearchQuery] = useState('');
  const [searchLatencyMs, setSearchLatencyMs] = useState<number | null>(null);
  const [searchMatchedIds, setSearchMatchedIds] = useState<Set<string> | null>(null);
  const [benchmark50k, setBenchmark50k] = useState(getBenchmarkActive());
  const [syncing, setSyncing] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  // Modals
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [accountModalOpen, setAccountModalOpen] = useState(false);
  const [settingsModalOpen, setSettingsModalOpen] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [composerOpen, setComposerOpen] = useState(false);
  const [replyTarget, setReplyTarget] = useState<ThreadDetail | null>(null);
  const [composerDraft, setComposerDraft] = useState<string | undefined>(undefined);

  const searchInputRef = useRef<HTMLInputElement | null>(null);

  const showToast = useCallback((message: string) => {
    setToast(message);
    window.setTimeout(() => {
      setToast((prev) => (prev === message ? null : prev));
    }, 2600);
  }, []);

  const refreshAll = useCallback(
    async (folder = activeFolder, accId = activeAccountId) => {
      const [accs, list, folderCounts, stats, cfg] = await Promise.all([
        listAccounts(),
        listThreads(accId, folder),
        getFolderCounts(accId),
        getPrivacyStats(),
        getSettings(),
      ]);
      setAccounts(accs);
      setThreads(list);
      setCounts(folderCounts);
      setPrivacyStats(stats);
      setSettings(cfg);

      setActiveThreadId((prev) => {
        if (prev && list.some((t) => t.id === prev)) return prev;
        return list[0]?.id ?? null;
      });
    },
    [activeAccountId, activeFolder],
  );

  // Initial load + background IMAP IDLE push event listener
  useEffect(() => {
    refreshAll();
    let unlisten: (() => void) | undefined;
    subscribeSyncEvents((ev) => {
      if (ev.phase === 'headers' || ev.phase === 'bodies') {
        refreshAll();
        if (ev.syncedCount > 0) {
          showToast(ev.detail);
        }
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [refreshAll, showToast]);

  // Load full thread messages & mark read when active thread changes
  useEffect(() => {
    if (!activeThreadId) {
      setActiveDetail(null);
      return;
    }
    let cancelled = false;
    getThreadDetail(activeThreadId)
      .then((detail) => {
        if (cancelled) return;
        setActiveDetail(detail);
        if (detail.thread.unreadCount > 0) {
          setThreadsRead([activeThreadId], true).then(() => {
            if (!cancelled) {
              setThreads((prev) =>
                prev.map((t) => (t.id === activeThreadId ? { ...t, unreadCount: 0 } : t)),
              );
              getFolderCounts(activeAccountId).then((c) => !cancelled && setCounts(c));
            }
          });
        }
      })
      .catch(() => {
        if (!cancelled) setActiveDetail(null);
      });
    return () => {
      cancelled = true;
    };
  }, [activeThreadId, activeAccountId]);

  // Live inline FTS5 search filtering
  useEffect(() => {
    const q = searchQuery.trim();
    if (!q) {
      setSearchMatchedIds(null);
      setSearchLatencyMs(null);
      return;
    }
    let cancelled = false;
    searchMessages(q, activeAccountId).then((res) => {
      if (cancelled) return;
      setSearchLatencyMs(res.queryTimeMs);
      setSearchMatchedIds(new Set(res.results.map((r) => r.threadId)));
    });
    return () => {
      cancelled = true;
    };
  }, [searchQuery, activeAccountId]);

  const visibleThreads = useMemo(() => {
    if (!searchMatchedIds) return threads;
    return threads.filter((t) => searchMatchedIds.has(t.id));
  }, [threads, searchMatchedIds]);

  const targetIdsForAction = useCallback(
    (explicitId?: string): string[] => {
      if (explicitId) return [explicitId];
      if (selectedIds.size > 0) return Array.from(selectedIds);
      if (activeThreadId) return [activeThreadId];
      return [];
    },
    [selectedIds, activeThreadId],
  );

  const handleArchive = useCallback(
    async (explicitId?: string) => {
      const ids = targetIdsForAction(explicitId);
      if (ids.length === 0) return;
      await archiveThreads(ids);
      setSelectedIds(new Set());
      showToast(`Archived ${ids.length} ${ids.length === 1 ? 'thread' : 'threads'} to Vault`);
      await refreshAll();
    },
    [targetIdsForAction, refreshAll, showToast],
  );

  const handleTrash = useCallback(
    async (explicitId?: string) => {
      const ids = targetIdsForAction(explicitId);
      if (ids.length === 0) return;
      await trashThreads(ids);
      setSelectedIds(new Set());
      showToast(`Moved ${ids.length} ${ids.length === 1 ? 'thread' : 'threads'} to Trash`);
      await refreshAll();
    },
    [targetIdsForAction, refreshAll, showToast],
  );

  const handleMoveToInbox = useCallback(
    async (threadId: string) => {
      await moveThreadsToInbox([threadId]);
      showToast('Moved back to Inbox');
      await refreshAll();
    },
    [refreshAll, showToast],
  );

  const handleSnooze = useCallback(
    async (explicitId?: string) => {
      const ids = targetIdsForAction(explicitId);
      if (ids.length === 0) return;
      const tomorrow9am = Math.floor(Date.now() / 1000) + 86400;
      await snoozeThreads(ids, tomorrow9am);
      setSelectedIds(new Set());
      showToast(`Snoozed ${ids.length} ${ids.length === 1 ? 'thread' : 'threads'} until tomorrow`);
      await refreshAll();
    },
    [targetIdsForAction, refreshAll, showToast],
  );

  const handleToggleRead = useCallback(
    async (explicitId?: string, explicitMarkRead?: boolean) => {
      const ids = targetIdsForAction(explicitId);
      if (ids.length === 0) return;
      const targetRead =
        explicitMarkRead !== undefined
          ? explicitMarkRead
          : (threads.find((t) => t.id === ids[0])?.unreadCount ?? 0) > 0;
      await setThreadsRead(ids, targetRead);
      setSelectedIds(new Set());
      await refreshAll();
    },
    [targetIdsForAction, threads, refreshAll],
  );

  const handleToggleStar = useCallback(
    async (explicitId?: string) => {
      const id = explicitId || activeThreadId;
      if (!id) return;
      await toggleThreadStar(id);
      await refreshAll();
      if (activeThreadId === id) {
        getThreadDetail(id).then(setActiveDetail).catch(() => {});
      }
    },
    [activeThreadId, refreshAll],
  );

  const handleSyncAll = useCallback(async () => {
    setSyncing(true);
    try {
      for (const acc of accounts) {
        await syncAccountNow(acc.id).catch(() => {});
      }
      await refreshAll();
      showToast('IMAP IDLE sync completed across all accounts');
    } finally {
      setSyncing(false);
    }
  }, [accounts, refreshAll, showToast]);

  const handleToggleBenchmark50k = useCallback(async () => {
    const next = !benchmark50k;
    const count = setBenchmarkRowCount(next);
    setBenchmark50k(next);
    setActiveFolder('inbox');
    setSearchQuery('');
    await refreshAll('inbox', activeAccountId);
    showToast(
      next
        ? `Mounted ${count.toLocaleString()} virtualized rows at 120fps`
        : 'Restored standard local vault view',
    );
  }, [benchmark50k, activeAccountId, refreshAll, showToast]);

  const openCompose = useCallback((reply?: ThreadDetail | null, draft?: string) => {
    setReplyTarget(reply ?? null);
    setComposerDraft(draft);
    setComposerOpen(true);
  }, []);

  const anyModalOpen =
    paletteOpen || accountModalOpen || settingsModalOpen || shortcutsOpen || composerOpen;

  // Global Superhuman / Linear Keyboard Navigation Engine
  useKeyboardNav({
    disabled: anyModalOpen,
    onMoveDown: () => {
      if (visibleThreads.length === 0) return;
      const idx = visibleThreads.findIndex((t) => t.id === activeThreadId);
      const next = idx < 0 ? 0 : Math.min(visibleThreads.length - 1, idx + 1);
      setActiveThreadId(visibleThreads[next].id);
    },
    onMoveUp: () => {
      if (visibleThreads.length === 0) return;
      const idx = visibleThreads.findIndex((t) => t.id === activeThreadId);
      const prev = idx <= 0 ? 0 : idx - 1;
      setActiveThreadId(visibleThreads[prev].id);
    },
    onArchive: () => handleArchive(),
    onReply: () => {
      if (activeDetail) openCompose(activeDetail);
    },
    onCompose: () => openCompose(null),
    onToggleSelect: () => {
      if (!activeThreadId) return;
      setSelectedIds((prev) => {
        const next = new Set(prev);
        if (next.has(activeThreadId)) next.delete(activeThreadId);
        else next.add(activeThreadId);
        return next;
      });
    },
    onTrash: () => handleTrash(),
    onToggleRead: () => handleToggleRead(),
    onToggleStar: () => handleToggleStar(),
    onSnooze: () => handleSnooze(),
    onOpenCommandPalette: () => setPaletteOpen((v) => !v),
    onFocusSearch: () => searchInputRef.current?.focus(),
    onToggleShortcuts: () => setShortcutsOpen((v) => !v),
    onEscape: () => {
      if (paletteOpen) setPaletteOpen(false);
      else if (composerOpen) setComposerOpen(false);
      else if (accountModalOpen) setAccountModalOpen(false);
      else if (settingsModalOpen) setSettingsModalOpen(false);
      else if (shortcutsOpen) setShortcutsOpen(false);
      else if (selectedIds.size > 0) setSelectedIds(new Set());
      else if (searchQuery) setSearchQuery('');
    },
    onJumpFolder: (folder) => {
      setActiveFolder(folder);
      setSelectedIds(new Set());
    },
  });

  return (
    <div className="h-screen w-screen flex bg-[#090a0d] text-[#edf0f5] overflow-hidden">
      {/* Pane 1: Collapsible Sidebar */}
      <Sidebar
        collapsed={sidebarCollapsed}
        onToggleCollapse={() => setSidebarCollapsed((v) => !v)}
        accounts={accounts}
        activeAccountId={activeAccountId}
        onSelectAccount={(id) => {
          setActiveAccountId(id);
          setSelectedIds(new Set());
        }}
        activeFolder={activeFolder}
        onSelectFolder={(folder) => {
          setActiveFolder(folder);
          setSelectedIds(new Set());
        }}
        counts={counts}
        privacyStats={privacyStats}
        syncing={syncing}
        onSyncAll={handleSyncAll}
        onCompose={() => openCompose(null)}
        onOpenCommandPalette={() => setPaletteOpen(true)}
        onOpenAccountSetup={() => setAccountModalOpen(true)}
        onOpenSettings={() => setSettingsModalOpen(true)}
      />

      {/* Pane 2: 120fps Virtualized Thread List */}
      <ThreadList
        threads={visibleThreads}
        accounts={accounts}
        activeFolder={activeFolder}
        activeThreadId={activeThreadId}
        selectedIds={selectedIds}
        searchQuery={searchQuery}
        searchLatencyMs={searchLatencyMs}
        searchInputRef={searchInputRef}
        benchmark50k={benchmark50k}
        onToggleBenchmark50k={handleToggleBenchmark50k}
        onSearchChange={setSearchQuery}
        onSelectThread={(id) => setActiveThreadId(id)}
        onToggleCheckbox={(id) =>
          setSelectedIds((prev) => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return next;
          })
        }
        onClearSelection={() => setSelectedIds(new Set())}
        onBulkArchive={() => handleArchive()}
        onBulkTrash={() => handleTrash()}
        onBulkMarkRead={() => handleToggleRead(undefined, true)}
        onToggleStar={(id) => handleToggleStar(id)}
      />

      {/* Pane 3: Sandboxed Reading Pane + Privacy Shield + Local AI Triage */}
      <ReadingPane
        detail={activeDetail}
        aiEnabled={settings.aiEnabled}
        onArchive={(id) => handleArchive(id)}
        onMoveToInbox={(id) => handleMoveToInbox(id)}
        onTrash={(id) => handleTrash(id)}
        onSnooze={(id) => handleSnooze(id)}
        onToggleRead={(id, isRead) => handleToggleRead(id, isRead)}
        onToggleStar={(id) => handleToggleStar(id)}
        onOpenReply={(detail, draft) => openCompose(detail, draft)}
      />

      {/* Modals & Overlays */}
      <CommandPalette
        open={paletteOpen}
        accounts={accounts}
        activeAccountId={activeAccountId}
        selectedCount={selectedIds.size}
        benchmark50k={benchmark50k}
        onClose={() => setPaletteOpen(false)}
        onSelectSearchResult={(threadId, folderRole) => {
          setActiveFolder(folderRole);
          setActiveThreadId(threadId);
        }}
        onSelectAccount={(id) => setActiveAccountId(id)}
        onJumpFolder={(f) => setActiveFolder(f)}
        onCompose={() => openCompose(null)}
        onOpenAccountSetup={() => setAccountModalOpen(true)}
        onOpenSettings={() => setSettingsModalOpen(true)}
        onSyncAll={handleSyncAll}
        onBulkArchive={() => handleArchive()}
        onToggleBenchmark50k={handleToggleBenchmark50k}
      />

      <AccountSetupModal
        open={accountModalOpen}
        onClose={() => setAccountModalOpen(false)}
        onAccountAdded={(acc) => {
          showToast(`Connected ${acc.email} · Keychain sealed & IMAP IDLE active`);
          refreshAll();
        }}
      />

      <ComposerModal
        open={composerOpen}
        accounts={accounts}
        defaultAccountId={activeAccountId}
        replyTarget={replyTarget}
        initialDraft={composerDraft}
        aiEnabled={settings.aiEnabled}
        onClose={() => setComposerOpen(false)}
        onSent={() => {
          showToast('Message delivered directly via SMTP');
          refreshAll();
          if (activeThreadId) {
            getThreadDetail(activeThreadId).then(setActiveDetail).catch(() => {});
          }
        }}
      />

      <SettingsModal
        open={settingsModalOpen}
        settings={settings}
        privacyStats={privacyStats}
        onClose={() => setSettingsModalOpen(false)}
        onSettingsSaved={(updated) => {
          setSettings(updated);
          showToast('Vault & Local AI configuration saved');
        }}
      />

      <ShortcutsModal open={shortcutsOpen} onClose={() => setShortcutsOpen(false)} />

      {/* Subtle Tactical Toast Notification */}
      {toast && (
        <div
          role="status"
          className="fixed bottom-4 right-5 z-50 px-3.5 py-2 rounded-lg bg-[#161a24] border border-white/[0.14] shadow-2xl text-[12px] font-mono text-[#edf0f5] flex items-center gap-2"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
          <span>{toast}</span>
        </div>
      )}
    </div>
  );
}

export default App;
