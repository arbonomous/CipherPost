import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  Account,
  AiDraftResponse,
  AiHealthResponse,
  AiSummaryResponse,
  AppSettings,
  ComposePayload,
  ConnectionTestResult,
  FolderCounts,
  MessageDetail,
  NewAccountInput,
  PrivacyStats,
  SearchResponse,
  SyncEventPayload,
  ThreadDetail,
  ThreadSummary,
} from '../types/mail';
import {
  buildSyntheticThreadDetail,
  generateBenchmarkThreads,
  INITIAL_DEMO_ACCOUNTS,
  INITIAL_DEMO_THREAD_DETAILS,
} from './demoData';

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// ---------------------------------------------------------------------------
// In-Browser Preview Fallback Engine (used when running `vite dev` outside Tauri)
// Mirrors the exact Rust SQLCipher + FTS5 + Sanitizer + Local AI IPC contracts.
// ---------------------------------------------------------------------------
let mockAccounts: Account[] = structuredClone(INITIAL_DEMO_ACCOUNTS);
let mockThreadDetails: Record<string, ThreadDetail> = structuredClone(INITIAL_DEMO_THREAD_DETAILS);
let mockBenchmarkThreads: ThreadSummary[] = [];
let mockSettings: AppSettings = {
  aiEnabled: true,
  aiEndpoint: 'http://localhost:11434',
  aiModel: 'llama3.2:latest',
  blockRemoteImages: true,
  stripTrackers: true,
  autoVerifyPgp: true,
};

function allMockThreads(): ThreadSummary[] {
  const base = Object.values(mockThreadDetails).map((d) => d.thread);
  return [...base, ...mockBenchmarkThreads].sort((a, b) => b.lastMessageAt - a.lastMessageAt);
}

export function setBenchmarkRowCount(enabled: boolean): number {
  mockBenchmarkThreads = enabled ? generateBenchmarkThreads(50_000) : [];
  return mockBenchmarkThreads.length;
}

export function getBenchmarkActive(): boolean {
  return mockBenchmarkThreads.length > 0;
}

// ---------------------------------------------------------------------------
// Typed IPC Bridge
// ---------------------------------------------------------------------------

export async function listAccounts(): Promise<Account[]> {
  if (isTauriRuntime()) {
    return invoke<Account[]>('list_accounts');
  }
  const threads = allMockThreads();
  return mockAccounts.map((acc) => ({
    ...acc,
    unreadCount: threads.filter(
      (t) => t.accountId === acc.id && t.folderRole === 'inbox' && t.unreadCount > 0,
    ).length,
  }));
}

export async function testAccountConnection(input: NewAccountInput): Promise<ConnectionTestResult> {
  if (isTauriRuntime()) {
    return invoke<ConnectionTestResult>('test_account_connection', { input });
  }
  // Realistic simulated TLS 1.3 handshake diagnostic in browser preview mode
  await new Promise((r) => setTimeout(r, 420));
  if (!input.email.includes('@') || !input.imapHost.trim()) {
    return {
      success: false,
      imapTlsVersion: null,
      imapCapabilities: [],
      smtpVerified: false,
      steps: [
        {
          step: 'IMAP TLS Handshake',
          status: 'failed',
          detail: 'Valid email address and IMAP hostname are required',
          latencyMs: 4,
        },
      ],
      error: 'Invalid account configuration',
    };
  }
  const isLocalBridge = input.imapHost === '127.0.0.1' || input.imapHost === 'localhost';
  return {
    success: true,
    imapTlsVersion: 'TLSv1_3 (TLS13_AES_256_GCM_SHA384)',
    imapCapabilities: ['IMAP4rev1', 'IDLE', 'CONDSTORE', 'UIDPLUS', 'MOVE', 'UTF8=ACCEPT'],
    smtpVerified: true,
    steps: [
      {
        step: 'IMAP TLS Handshake',
        status: 'passed',
        detail: `${input.imapHost}:${input.imapPort} — TLSv1.3 (AES_256_GCM_SHA384)${isLocalBridge ? ' · Localhost Bridge Cert' : ''}`,
        latencyMs: isLocalBridge ? 3 : 38,
      },
      {
        step: 'IMAP Auth & Capabilities',
        status: 'passed',
        detail: `Authenticated (${input.authType}) · IDLE push: supported`,
        latencyMs: isLocalBridge ? 5 : 44,
      },
      {
        step: 'INBOX Access',
        status: 'passed',
        detail: 'INBOX ready (direct client-to-server session verified)',
        latencyMs: isLocalBridge ? 2 : 19,
      },
      {
        step: 'SMTP Delivery & TLS',
        status: 'passed',
        detail: `${input.smtpHost}:${input.smtpPort} (${input.smtpTls}) authenticated`,
        latencyMs: isLocalBridge ? 4 : 52,
      },
    ],
    error: null,
  };
}

export async function addAccount(input: NewAccountInput): Promise<Account> {
  if (isTauriRuntime()) {
    return invoke<Account>('add_account', { input });
  }
  const id = `acc-${Date.now()}`;
  const created: Account = {
    id,
    email: input.email.trim(),
    displayName: input.displayName.trim() || input.email.trim(),
    provider: input.provider,
    authType: input.authType,
    imapHost: input.imapHost,
    imapPort: input.imapPort,
    imapTls: input.imapTls,
    smtpHost: input.smtpHost,
    smtpPort: input.smtpPort,
    smtpTls: input.smtpTls,
    accentColor:
      input.accentColor ||
      (input.provider === 'gmail'
        ? '#ef4444'
        : input.provider === 'icloud'
          ? '#38bdf8'
          : input.provider === 'proton_bridge'
            ? '#10b981'
            : '#f59e0b'),
    syncStatus: 'idle',
    lastSyncedAt: Math.floor(Date.now() / 1000),
    unreadCount: 0,
  };
  mockAccounts = [...mockAccounts, created];
  return created;
}

export async function removeAccount(accountId: string): Promise<void> {
  if (isTauriRuntime()) {
    return invoke<void>('remove_account', { accountId });
  }
  mockAccounts = mockAccounts.filter((a) => a.id !== accountId);
}

export async function syncAccountNow(accountId: string): Promise<number> {
  if (isTauriRuntime()) {
    return invoke<number>('sync_account_now', { accountId });
  }
  await new Promise((r) => setTimeout(r, 320));
  mockAccounts = mockAccounts.map((a) =>
    a.id === accountId ? { ...a, lastSyncedAt: Math.floor(Date.now() / 1000), syncStatus: 'idle' } : a,
  );
  return 0;
}

export async function listThreads(
  accountId: string | null,
  folderRole: string,
  limit = 60000,
): Promise<ThreadSummary[]> {
  if (isTauriRuntime()) {
    const dbThreads = await invoke<ThreadSummary[]>('list_threads', {
      accountId,
      folderRole,
      limit,
      offset: 0,
    });
    if (folderRole === 'inbox' && mockBenchmarkThreads.length > 0) {
      const bench = accountId
        ? mockBenchmarkThreads.filter((t) => t.accountId === accountId)
        : mockBenchmarkThreads;
      return [...dbThreads, ...bench];
    }
    return dbThreads;
  }
  return allMockThreads().filter(
    (t) => t.folderRole === folderRole && (!accountId || t.accountId === accountId),
  );
}

export async function getThreadDetail(threadId: string): Promise<ThreadDetail> {
  if (threadId.startsWith('bench-')) {
    const found = mockBenchmarkThreads.find((t) => t.id === threadId);
    if (found) return buildSyntheticThreadDetail(found);
  }
  if (isTauriRuntime()) {
    return invoke<ThreadDetail>('get_thread_detail', { threadId });
  }
  const detail = mockThreadDetails[threadId];
  if (!detail) {
    throw new Error(`Thread ${threadId} not found`);
  }
  return structuredClone(detail);
}

export async function searchMessages(
  query: string,
  accountId: string | null,
): Promise<SearchResponse> {
  if (isTauriRuntime()) {
    return invoke<SearchResponse>('search_messages', { query, accountId, limit: 40 });
  }
  const t0 = performance.now();
  const q = query.trim().toLowerCase();
  if (!q) {
    return { query, queryTimeMs: 0, totalHits: 0, results: [] };
  }
  const tokens = q.split(/\s+/).filter(Boolean);
  const results: SearchResponse['results'] = [];
  for (const t of allMockThreads()) {
    if (accountId && t.accountId !== accountId) continue;
    const hay = `${t.subject} ${t.lastSenderName} ${t.lastSenderEmail} ${t.snippet}`.toLowerCase();
    if (tokens.every((tok) => hay.includes(tok))) {
      const escaped = tokens[0].replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const highlighted = t.snippet.replace(new RegExp(`(${escaped})`, 'ig'), '<mark>$1</mark>');
      results.push({
        threadId: t.id,
        messageId: `${t.id}-msg`,
        accountId: t.accountId,
        folderRole: t.folderRole,
        subject: t.subject,
        senderName: t.lastSenderName,
        senderEmail: t.lastSenderEmail,
        highlightedSnippet: highlighted,
        receivedAt: t.lastMessageAt,
        pgpStatus: t.pgpStatus,
        trackersBlocked: t.trackersBlocked,
        rank: -1,
      });
      if (results.length >= 40) break;
    }
  }
  const elapsed = Math.round((performance.now() - t0) * 100) / 100;
  return { query, queryTimeMs: elapsed, totalHits: results.length, results };
}

export async function archiveThreads(threadIds: string[]): Promise<number> {
  if (isTauriRuntime()) {
    const realIds = threadIds.filter((id) => !id.startsWith('bench-'));
    mockBenchmarkThreads = mockBenchmarkThreads.filter((t) => !threadIds.includes(t.id));
    if (realIds.length > 0) {
      await invoke<number>('archive_threads', { threadIds: realIds });
    }
    return threadIds.length;
  }
  for (const id of threadIds) {
    if (mockThreadDetails[id]) {
      mockThreadDetails[id].thread.folderRole = 'archive';
      mockThreadDetails[id].thread.snoozedUntil = null;
    }
  }
  mockBenchmarkThreads = mockBenchmarkThreads.filter((t) => !threadIds.includes(t.id));
  return threadIds.length;
}

export async function trashThreads(threadIds: string[]): Promise<number> {
  if (isTauriRuntime()) {
    const realIds = threadIds.filter((id) => !id.startsWith('bench-'));
    mockBenchmarkThreads = mockBenchmarkThreads.filter((t) => !threadIds.includes(t.id));
    if (realIds.length > 0) {
      await invoke<number>('trash_threads', { threadIds: realIds });
    }
    return threadIds.length;
  }
  for (const id of threadIds) {
    if (mockThreadDetails[id]) {
      mockThreadDetails[id].thread.folderRole = 'trash';
    }
  }
  mockBenchmarkThreads = mockBenchmarkThreads.filter((t) => !threadIds.includes(t.id));
  return threadIds.length;
}

export async function moveThreadsToInbox(threadIds: string[]): Promise<number> {
  if (isTauriRuntime()) {
    return invoke<number>('move_threads_to_inbox', { threadIds });
  }
  for (const id of threadIds) {
    if (mockThreadDetails[id]) {
      mockThreadDetails[id].thread.folderRole = 'inbox';
      mockThreadDetails[id].thread.snoozedUntil = null;
    }
  }
  return threadIds.length;
}

export async function snoozeThreads(threadIds: string[], untilTs: number): Promise<number> {
  if (isTauriRuntime()) {
    return invoke<number>('snooze_threads', { threadIds, untilTs });
  }
  for (const id of threadIds) {
    if (mockThreadDetails[id]) {
      mockThreadDetails[id].thread.folderRole = 'snoozed';
      mockThreadDetails[id].thread.snoozedUntil = untilTs;
    }
  }
  return threadIds.length;
}

export async function setThreadsRead(threadIds: string[], isRead: boolean): Promise<number> {
  if (isTauriRuntime()) {
    const realIds = threadIds.filter((id) => !id.startsWith('bench-'));
    if (realIds.length > 0) {
      await invoke<number>('set_threads_read', { threadIds: realIds, isRead });
    }
    return threadIds.length;
  }
  for (const id of threadIds) {
    if (mockThreadDetails[id]) {
      mockThreadDetails[id].thread.unreadCount = isRead ? 0 : 1;
      for (const m of mockThreadDetails[id].messages) {
        m.isRead = isRead;
      }
    }
  }
  for (const b of mockBenchmarkThreads) {
    if (threadIds.includes(b.id)) {
      b.unreadCount = isRead ? 0 : 1;
    }
  }
  return threadIds.length;
}

export async function toggleThreadStar(threadId: string): Promise<boolean> {
  if (isTauriRuntime() && !threadId.startsWith('bench-')) {
    return invoke<boolean>('toggle_thread_star', { threadId });
  }
  if (mockThreadDetails[threadId]) {
    const next = !mockThreadDetails[threadId].thread.isStarred;
    mockThreadDetails[threadId].thread.isStarred = next;
    return next;
  }
  const bench = mockBenchmarkThreads.find((t) => t.id === threadId);
  if (bench) {
    bench.isStarred = !bench.isStarred;
    return bench.isStarred;
  }
  return false;
}

export async function sendMessage(payload: ComposePayload): Promise<string> {
  if (isTauriRuntime()) {
    return invoke<string>('send_message', { payload });
  }
  await new Promise((r) => setTimeout(r, 260));
  const acc = mockAccounts.find((a) => a.id === payload.accountId) || mockAccounts[0];
  const now = Math.floor(Date.now() / 1000);
  const msgId = `msg-${Date.now()}`;
  const pgpStatus =
    payload.pgpEncrypt && payload.pgpSign
      ? 'encrypted_verified'
      : payload.pgpEncrypt
        ? 'encrypted'
        : payload.pgpSign
          ? 'signed_verified'
          : 'none';

  const safeHtml = `<div style="white-space:pre-wrap;">${payload.bodyText
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')}</div>`;

  const newMsg: MessageDetail = {
    id: msgId,
    threadId: payload.threadId || `thr-sent-${Date.now()}`,
    accountId: acc.id,
    folderRole: 'sent',
    messageIdHeader: `<${msgId}@cipherpost.local>`,
    subject: payload.subject,
    senderName: acc.displayName,
    senderEmail: acc.email,
    to: payload.to.map((e) => ({ name: e, email: e })),
    cc: payload.cc.map((e) => ({ name: e, email: e })),
    bodyText: payload.bodyText,
    bodyHtmlSanitized: safeHtml,
    bodyHtmlWithAssets: safeHtml,
    blockedTrackers: [],
    trackersBlockedCount: 0,
    remoteAssetsCount: 0,
    pgpStatus,
    pgpFingerprint: pgpStatus !== 'none' ? '3A80 99F1 C4B2 7701 11E4 9A02 4C8D 1022' : null,
    pgpSigner: pgpStatus !== 'none' ? `${acc.displayName} <${acc.email}>` : null,
    isRead: true,
    isStarred: false,
    isBodyFetched: true,
    sizeBytes: payload.bodyText.length,
    receivedAt: now,
    attachments: [],
  };

  if (payload.threadId && mockThreadDetails[payload.threadId]) {
    const existing = mockThreadDetails[payload.threadId];
    existing.messages.push({ ...newMsg });
    existing.thread.messageCount = existing.messages.length;
    existing.thread.lastMessageAt = now;
    existing.thread.snippet = payload.bodyText.slice(0, 140);
  } else {
    mockThreadDetails[newMsg.threadId] = {
      thread: {
        id: newMsg.threadId,
        accountId: acc.id,
        folderRole: 'sent',
        subject: payload.subject || '(No Subject)',
        snippet: payload.bodyText.slice(0, 140),
        participants: [{ name: acc.displayName, email: acc.email }],
        lastSenderName: acc.displayName,
        lastSenderEmail: acc.email,
        messageCount: 1,
        unreadCount: 0,
        isStarred: false,
        hasAttachments: false,
        pgpStatus,
        trackersBlocked: 0,
        snoozedUntil: null,
        lastMessageAt: now,
      },
      messages: [{ ...newMsg }],
    };
  }
  return msgId;
}

export async function getFolderCounts(accountId: string | null): Promise<FolderCounts> {
  if (isTauriRuntime()) {
    const counts = await invoke<FolderCounts>('get_folder_counts', { accountId });
    if (mockBenchmarkThreads.length > 0) {
      const bench = accountId
        ? mockBenchmarkThreads.filter((t) => t.accountId === accountId)
        : mockBenchmarkThreads;
      return {
        ...counts,
        inboxTotal: counts.inboxTotal + bench.length,
        inboxUnread: counts.inboxUnread + bench.filter((t) => t.unreadCount > 0).length,
      };
    }
    return counts;
  }
  const filtered = allMockThreads().filter((t) => !accountId || t.accountId === accountId);
  return {
    inboxUnread: filtered.filter((t) => t.folderRole === 'inbox' && t.unreadCount > 0).length,
    inboxTotal: filtered.filter((t) => t.folderRole === 'inbox').length,
    sentTotal: filtered.filter((t) => t.folderRole === 'sent').length,
    draftsTotal: filtered.filter((t) => t.folderRole === 'drafts').length,
    snoozedTotal: filtered.filter((t) => t.folderRole === 'snoozed').length,
    archiveTotal: filtered.filter((t) => t.folderRole === 'archive').length,
    trashTotal: filtered.filter((t) => t.folderRole === 'trash').length,
  };
}

export async function getPrivacyStats(): Promise<PrivacyStats> {
  if (isTauriRuntime()) {
    return invoke<PrivacyStats>('get_privacy_stats');
  }
  const all = allMockThreads();
  return {
    totalTrackersBlocked: all.reduce((sum, t) => sum + t.trackersBlocked, 0),
    totalMessagesIndexed: all.length,
    pgpVerifiedCount: all.filter((t) => t.pgpStatus !== 'none').length,
    sqlcipherActive: true,
    keychainBackend: 'macOS Keychain (Security.framework)',
    fts5Ready: true,
  };
}

export async function getSettings(): Promise<AppSettings> {
  if (isTauriRuntime()) {
    return invoke<AppSettings>('get_settings');
  }
  return { ...mockSettings };
}

export async function saveSettings(settings: AppSettings): Promise<AppSettings> {
  if (isTauriRuntime()) {
    return invoke<AppSettings>('save_settings', { settings });
  }
  const parsed = new URL(settings.aiEndpoint);
  if (!['localhost', '127.0.0.1', '[::1]'].includes(parsed.hostname)) {
    throw new Error(
      `Zero-Trust Policy Violation: AI endpoint '${parsed.hostname}' is not local loopback.`,
    );
  }
  mockSettings = { ...settings };
  return { ...mockSettings };
}

export async function checkLocalAi(endpoint?: string): Promise<AiHealthResponse> {
  if (isTauriRuntime()) {
    return invoke<AiHealthResponse>('check_local_ai', { endpoint });
  }
  const target = (endpoint || mockSettings.aiEndpoint).replace(/\/$/, '');
  const t0 = performance.now();
  try {
    const resp = await fetch(`${target}/api/tags`, { signal: AbortSignal.timeout(2500) });
    if (resp.ok) {
      const data = await resp.json();
      return {
        reachable: true,
        endpoint: target,
        availableModels: (data.models || []).map((m: { name: string }) => m.name),
        latencyMs: Math.round(performance.now() - t0),
        error: null,
      };
    }
  } catch {
    // Fallback offline note
  }
  return {
    reachable: false,
    endpoint: target,
    availableModels: ['llama3.2:latest', 'qwen2.5:7b', 'mlx-community/Mistral-7B-Instruct'],
    latencyMs: Math.round(performance.now() - t0),
    error: `Ollama/MLX server not running at ${target}. Run \`ollama serve\` locally to enable live on-device inference.`,
  };
}

export async function aiSummarizeThread(threadId: string): Promise<AiSummaryResponse> {
  if (isTauriRuntime() && !threadId.startsWith('bench-')) {
    return invoke<AiSummaryResponse>('ai_summarize_thread', { threadId });
  }
  await new Promise((r) => setTimeout(r, 340));
  const detail = await getThreadDetail(threadId);
  const last = detail.messages[detail.messages.length - 1];
  const sentences = last.bodyText
    .replace(/\n+/g, ' ')
    .split(/(?<=[.!?])\s+/)
    .filter((s) => s.length > 18);

  return {
    threadId,
    bullets: [
      sentences[0]?.slice(0, 125) || `Summary from ${last.senderName} regarding "${detail.thread.subject}".`,
      sentences[1]?.slice(0, 125) ||
        'Review action items and confirm reply directly from your local encrypted client.',
    ],
    actionRequired: /action|confirm|reply|review|friday|po number/i.test(last.bodyText),
    modelUsed: `${mockSettings.aiModel} (local)`,
    latencyMs: 312,
  };
}

export async function aiDraftReply(
  threadId: string,
  instruction?: string,
): Promise<AiDraftResponse> {
  if (isTauriRuntime() && !threadId.startsWith('bench-')) {
    return invoke<AiDraftResponse>('ai_draft_reply', { threadId, instruction });
  }
  await new Promise((r) => setTimeout(r, 380));
  const detail = await getThreadDetail(threadId);
  const last = detail.messages[detail.messages.length - 1];
  const firstName = last.senderName.split(' ')[0] || 'there';
  return {
    draftText: `Hi ${firstName},\n\nThanks for the update on "${detail.thread.subject}". Everything looks solid on our end—I'll proceed with the next steps and follow up once finalized.${instruction ? `\n\n(${instruction})` : ''}\n\nBest,\nElena`,
    modelUsed: `${mockSettings.aiModel} (local)`,
    latencyMs: 365,
  };
}

export async function subscribeSyncEvents(
  handler: (event: SyncEventPayload) => void,
): Promise<UnlistenFn> {
  if (isTauriRuntime()) {
    return listen<SyncEventPayload>('cipherpost://sync', (ev) => handler(ev.payload));
  }
  return () => {};
}
