import { useEffect, useState } from 'react';
import {
  Archive,
  ChevronDown,
  ChevronUp,
  Clock,
  Cpu,
  Image as ImageIcon,
  Inbox,
  Lock,
  MailOpen,
  Paperclip,
  Reply,
  ShieldAlert,
  ShieldCheck,
  Sparkles,
  Star,
  Trash2,
} from 'lucide-react';
import { formatBytes, formatFullTimestamp, getInitials } from '../lib/format';
import { aiDraftReply, aiSummarizeThread } from '../lib/tauri';
import type {
  AiSummaryResponse,
  MessageDetail,
  PgpStatus,
  ThreadDetail,
} from '../types/mail';
import { SandboxedEmailFrame } from './SandboxedEmailFrame';

interface ReadingPaneProps {
  detail: ThreadDetail | null;
  aiEnabled: boolean;
  onArchive: (threadId: string) => void;
  onMoveToInbox: (threadId: string) => void;
  onTrash: (threadId: string) => void;
  onSnooze: (threadId: string) => void;
  onToggleRead: (threadId: string, isRead: boolean) => void;
  onToggleStar: (threadId: string) => void;
  onOpenReply: (detail: ThreadDetail, prefillDraft?: string) => void;
}

function PgpBadge({
  status,
  fingerprint,
  signer,
}: {
  status: PgpStatus;
  fingerprint: string | null;
  signer: string | null;
}) {
  const [expanded, setExpanded] = useState(false);
  if (status === 'none') return null;

  const isVerified = status === 'signed_verified' || status === 'encrypted_verified';
  const isEncrypted = status === 'encrypted' || status === 'encrypted_verified';

  const label =
    status === 'encrypted_verified'
      ? 'PGP Encrypted · Verified'
      : status === 'encrypted'
        ? 'PGP Encrypted'
        : status === 'signed_verified'
          ? 'PGP Verified'
          : 'PGP Signed (Unverified Key)';

  return (
    <div className="relative">
      <button
        onClick={() => setExpanded((v) => !v)}
        className={`h-6 px-2 rounded-md border text-[11px] font-mono flex items-center gap-1.5 transition-colors ${
          isVerified || isEncrypted
            ? 'bg-emerald-500/10 border-emerald-500/25 text-emerald-300 hover:bg-emerald-500/15'
            : 'bg-amber-500/10 border-amber-500/25 text-amber-300 hover:bg-amber-500/15'
        }`}
      >
        <Lock className="w-3 h-3" />
        <span>{label}</span>
      </button>

      {expanded && (
        <div className="absolute left-0 top-8 z-30 w-[340px] p-3 rounded-lg bg-[#131722] border border-white/[0.12] shadow-2xl space-y-2 text-[11.5px]">
          <div className="flex items-center justify-between">
            <span className="font-semibold text-[#f1f5f9]">Sequoia OpenPGP Verification</span>
            <span className="font-mono text-[10px] text-emerald-400">LOCAL</span>
          </div>
          {signer && (
            <div>
              <div className="text-[10px] font-mono uppercase text-[#596174]">Signer UID</div>
              <div className="text-[#dce2ec] selectable">{signer}</div>
            </div>
          )}
          {fingerprint && (
            <div>
              <div className="text-[10px] font-mono uppercase text-[#596174]">Key Fingerprint</div>
              <div className="font-mono text-[10.5px] text-emerald-300/90 break-all selectable">
                {fingerprint}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function MessageCard({
  message,
  defaultExpanded,
}: {
  message: MessageDetail;
  defaultExpanded: boolean;
}) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const [loadRemoteAssets, setLoadRemoteAssets] = useState(false);
  const [showTrackerAudit, setShowTrackerAudit] = useState(false);

  useEffect(() => {
    setExpanded(defaultExpanded);
    setLoadRemoteAssets(false);
    setShowTrackerAudit(false);
  }, [message.id, defaultExpanded]);

  const trackerCount = message.blockedTrackers.length || message.trackersBlockedCount;

  return (
    <article className="rounded-lg bg-[#11141c] border border-white/[0.068] overflow-hidden">
      {/* Message Header */}
      <div
        onClick={() => setExpanded((v) => !v)}
        className="px-4 py-3 flex items-start justify-between gap-3 cursor-pointer hover:bg-white/[0.02] transition-colors"
      >
        <div className="flex items-start gap-3 min-w-0">
          <div className="w-8 h-8 rounded-md bg-[#1a1f2b] border border-white/[0.09] flex items-center justify-center text-[11px] font-mono font-semibold text-[#cbd5e1] shrink-0">
            {getInitials(message.senderName)}
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="text-[13px] font-semibold text-[#f1f5f9] selectable">
                {message.senderName}
              </span>
              <span className="text-[11.5px] font-mono text-[#667085] selectable">
                &lt;{message.senderEmail}&gt;
              </span>
            </div>
            <div className="text-[11.5px] text-[#7c8599] mt-0.5 truncate">
              To:{' '}
              {message.to.length > 0
                ? message.to.map((r) => r.email).join(', ')
                : 'Undisclosed Recipients'}
              {message.cc.length > 0 && ` · Cc: ${message.cc.map((r) => r.email).join(', ')}`}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2 shrink-0">
          <span className="text-[11px] font-mono tabular-nums text-[#667085]">
            {formatFullTimestamp(message.receivedAt)}
          </span>
          {expanded ? (
            <ChevronUp className="w-3.5 h-3.5 text-[#596174]" />
          ) : (
            <ChevronDown className="w-3.5 h-3.5 text-[#596174]" />
          )}
        </div>
      </div>

      {expanded && (
        <div className="border-t border-white/[0.055]">
          {/* Active Privacy & Tracker Shield Strip */}
          <div className="px-4 py-2 bg-[#0d1017] border-b border-white/[0.05] flex items-center justify-between gap-2 flex-wrap">
            <div className="flex items-center gap-2 flex-wrap">
              {/* Subtle Tracker Shield Badge */}
              <button
                onClick={() => trackerCount > 0 && setShowTrackerAudit((v) => !v)}
                className={`h-6 px-2 rounded-md border text-[11px] font-mono flex items-center gap-1.5 transition-colors ${
                  trackerCount > 0
                    ? 'bg-amber-500/10 border-amber-500/25 text-amber-300 hover:bg-amber-500/15 cursor-pointer'
                    : 'bg-emerald-500/10 border-emerald-500/20 text-emerald-300/90 cursor-default'
                }`}
              >
                {trackerCount > 0 ? (
                  <>
                    <ShieldAlert className="w-3 h-3" />
                    <span>
                      {trackerCount} {trackerCount === 1 ? 'tracker' : 'trackers'} blocked
                    </span>
                    <ChevronDown
                      className={`w-3 h-3 transition-transform ${showTrackerAudit ? 'rotate-180' : ''}`}
                    />
                  </>
                ) : (
                  <>
                    <ShieldCheck className="w-3 h-3" />
                    <span>Privacy protected</span>
                  </>
                )}
              </button>

              <PgpBadge
                status={message.pgpStatus}
                fingerprint={message.pgpFingerprint}
                signer={message.pgpSigner}
              />
            </div>

            {/* One-Click Load Remote Assets Toggle */}
            {message.remoteAssetsCount > 0 && (
              <button
                id={`btn-load-assets-${message.id}`}
                onClick={() => setLoadRemoteAssets((v) => !v)}
                className={`h-6 px-2.5 rounded-md border text-[11px] font-medium flex items-center gap-1.5 transition-colors ${
                  loadRemoteAssets
                    ? 'bg-emerald-500/15 border-emerald-500/30 text-emerald-200'
                    : 'bg-white/[0.04] border-white/[0.09] text-[#cbd5e1] hover:bg-white/[0.08]'
                }`}
              >
                <ImageIcon className="w-3 h-3" />
                <span>
                  {loadRemoteAssets
                    ? 'Remote Assets Loaded (Trackers Still Stripped)'
                    : `Load Remote Assets (${message.remoteAssetsCount})`}
                </span>
              </button>
            )}
          </div>

          {/* Expandable Tracker Audit Log */}
          {showTrackerAudit && message.blockedTrackers.length > 0 && (
            <div className="px-4 py-2.5 bg-[#0b0d13] border-b border-white/[0.055] space-y-1.5">
              <div className="text-[10px] font-mono uppercase tracking-wider text-amber-300/80">
                Neutralized by Rust HTML Sanitizer (lol_html)
              </div>
              <div className="space-y-1">
                {message.blockedTrackers.map((tr, i) => (
                  <div
                    key={`${tr.url}-${i}`}
                    className="px-2.5 py-1.5 rounded bg-white/[0.025] border border-white/[0.05] flex items-center justify-between gap-3 text-[11px]"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300 uppercase">
                          {tr.category.replace('_', ' ')}
                        </span>
                        <span className="font-mono text-[#dce2ec] truncate">{tr.domain}</span>
                      </div>
                      <div className="text-[#7c8599] text-[10.5px] mt-0.5 truncate selectable">
                        {tr.reason} — <span className="font-mono text-[#596174]">{tr.url}</span>
                      </div>
                    </div>
                    <span className="font-mono text-[10px] text-[#596174] shrink-0">
                      &lt;{tr.element}&gt;
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Strictly Sandboxed Email Body Iframe */}
          <div className="px-5 py-4">
            <SandboxedEmailFrame
              html={loadRemoteAssets ? message.bodyHtmlWithAssets : message.bodyHtmlSanitized}
              allowRemoteAssets={loadRemoteAssets}
            />
          </div>

          {/* Attachments Shelf */}
          {message.attachments.length > 0 && (
            <div className="px-4 py-2.5 border-t border-white/[0.055] bg-[#0d1017] flex items-center gap-2 flex-wrap">
              {message.attachments.map((att) => (
                <div
                  key={att.id}
                  className="px-2.5 py-1.5 rounded-md bg-[#141822] border border-white/[0.08] flex items-center gap-2 text-[11.5px]"
                >
                  <Paperclip className="w-3.5 h-3.5 text-[#7c8599]" />
                  <span className="text-[#dce2ec] font-medium">{att.filename}</span>
                  <span className="font-mono text-[10.5px] text-[#667085]">
                    {formatBytes(att.sizeBytes)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </article>
  );
}

export function ReadingPane({
  detail,
  aiEnabled,
  onArchive,
  onMoveToInbox,
  onTrash,
  onSnooze,
  onToggleRead,
  onToggleStar,
  onOpenReply,
}: ReadingPaneProps) {
  const [summary, setSummary] = useState<AiSummaryResponse | null>(null);
  const [aiLoading, setAiLoading] = useState(false);
  const [aiDrafting, setAiDrafting] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);

  useEffect(() => {
    setSummary(null);
    setAiError(null);
  }, [detail?.thread.id]);

  if (!detail) {
    return (
      <main className="flex-1 h-full bg-[#0e1015] flex flex-col items-center justify-center p-8 text-center">
        <div className="w-12 h-12 rounded-xl bg-[#131720] border border-white/[0.08] flex items-center justify-center mb-4">
          <ShieldCheck className="w-6 h-6 text-emerald-400" />
        </div>
        <h2 className="text-[15px] font-semibold text-[#edf0f5] tracking-[-0.015em]">
          Zero-Trust Reading Vault
        </h2>
        <p className="text-[12.5px] text-[#667085] max-w-sm mt-1.5 leading-relaxed">
          Select a thread with <span className="kbd">J</span> / <span className="kbd">K</span> or
          press <span className="kbd">⌘K</span> to run sub-20ms SQLite FTS5 search across your
          encrypted local mail store.
        </p>
      </main>
    );
  }

  const { thread, messages } = detail;
  const isUnread = thread.unreadCount > 0;

  const handleSummarize = async () => {
    setAiLoading(true);
    setAiError(null);
    try {
      const res = await aiSummarizeThread(thread.id);
      setSummary(res);
    } catch (err) {
      setAiError(err instanceof Error ? err.message : String(err));
    } finally {
      setAiLoading(false);
    }
  };

  const handleAiDraft = async () => {
    setAiDrafting(true);
    setAiError(null);
    try {
      const res = await aiDraftReply(thread.id);
      onOpenReply(detail, res.draftText);
    } catch (err) {
      setAiError(err instanceof Error ? err.message : String(err));
    } finally {
      setAiDrafting(false);
    }
  };

  return (
    <main className="flex-1 h-full bg-[#0e1015] flex flex-col min-w-0">
      {/* Top Action Toolbar */}
      <header
        data-tauri-drag-region
        className="titlebar-drag h-12 px-5 border-b border-white/[0.055] flex items-center justify-between gap-3 shrink-0"
      >
        <div className="titlebar-no-drag flex items-center gap-1">
          {thread.folderRole !== 'archive' ? (
            <button
              id="btn-thread-archive"
              onClick={() => onArchive(thread.id)}
              title="Archive to Vault (E)"
              className="h-7 px-2.5 rounded-md text-[#9ba3b5] hover:text-[#edf0f5] hover:bg-white/[0.06] text-[12px] flex items-center gap-1.5 transition-colors"
            >
              <Archive className="w-3.5 h-3.5" />
              <span>Archive</span>
              <span className="kbd ml-0.5">E</span>
            </button>
          ) : (
            <button
              onClick={() => onMoveToInbox(thread.id)}
              className="h-7 px-2.5 rounded-md text-[#9ba3b5] hover:text-[#edf0f5] hover:bg-white/[0.06] text-[12px] flex items-center gap-1.5 transition-colors"
            >
              <Inbox className="w-3.5 h-3.5" />
              <span>Move to Inbox</span>
            </button>
          )}

          <button
            id="btn-thread-snooze"
            onClick={() => onSnooze(thread.id)}
            title="Snooze until tomorrow 9:00 AM (H)"
            className="h-7 px-2 rounded-md text-[#9ba3b5] hover:text-[#edf0f5] hover:bg-white/[0.06] text-[12px] flex items-center gap-1.5 transition-colors"
          >
            <Clock className="w-3.5 h-3.5" />
            <span className="hidden lg:inline">Snooze</span>
          </button>

          <button
            id="btn-thread-unread"
            onClick={() => onToggleRead(thread.id, isUnread)}
            title="Toggle Read/Unread (U)"
            className="h-7 px-2 rounded-md text-[#9ba3b5] hover:text-[#edf0f5] hover:bg-white/[0.06] text-[12px] flex items-center gap-1.5 transition-colors"
          >
            <MailOpen className="w-3.5 h-3.5" />
            <span className="hidden lg:inline">{isUnread ? 'Mark Read' : 'Mark Unread'}</span>
          </button>

          <button
            id="btn-thread-star"
            onClick={() => onToggleStar(thread.id)}
            title="Star (S)"
            className={`h-7 px-2 rounded-md hover:bg-white/[0.06] flex items-center transition-colors ${
              thread.isStarred ? 'text-amber-400' : 'text-[#9ba3b5] hover:text-[#edf0f5]'
            }`}
          >
            <Star className="w-3.5 h-3.5" fill={thread.isStarred ? 'currentColor' : 'none'} />
          </button>

          <button
            id="btn-thread-trash"
            onClick={() => onTrash(thread.id)}
            title="Move to Trash (#)"
            className="h-7 px-2 rounded-md text-[#9ba3b5] hover:text-rose-300 hover:bg-rose-500/10 flex items-center transition-colors"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Right: Local AI Triage Controls + Reply */}
        <div className="titlebar-no-drag flex items-center gap-1.5">
          {aiEnabled && (
            <>
              <button
                id="btn-ai-summarize"
                onClick={handleSummarize}
                disabled={aiLoading}
                title="Generate a 2-bullet summary on your device"
                className="h-7 px-2.5 rounded-md bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] text-[#cbd5e1] text-[11.5px] font-medium flex items-center gap-1.5 transition-colors disabled:opacity-50"
              >
                <Cpu className="w-3.5 h-3.5 text-emerald-400" />
                <span>{aiLoading ? 'Summarizing…' : 'Summarize'}</span>
              </button>

              <button
                id="btn-ai-draft"
                onClick={handleAiDraft}
                disabled={aiDrafting}
                title="Draft a reply on your device"
                className="h-7 px-2.5 rounded-md bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] text-[#cbd5e1] text-[11.5px] font-medium flex items-center gap-1.5 transition-colors disabled:opacity-50"
              >
                <Sparkles className="w-3.5 h-3.5 text-amber-300" />
                <span>{aiDrafting ? 'Drafting…' : 'Draft Reply'}</span>
              </button>
            </>
          )}

          <button
            id="btn-thread-reply"
            onClick={() => onOpenReply(detail)}
            className="h-7 px-3 rounded-md bg-[#edf0f5] hover:bg-white text-[#090a0d] text-[12px] font-medium flex items-center gap-1.5 transition-colors"
          >
            <Reply className="w-3.5 h-3.5" />
            <span>Reply</span>
            <span className="font-mono text-[10px] px-1 rounded bg-black/10">R</span>
          </button>
        </div>
      </header>

      {/* Scrollable Thread Conversation Body */}
      <div className="flex-1 overflow-y-auto px-6 py-5 space-y-4">
        {/* Subject Header */}
        <div className="space-y-2">
          <div className="flex items-start justify-between gap-4">
            <h2 className="text-[18px] font-semibold tracking-[-0.02em] text-[#f5f7fb] leading-snug selectable">
              {thread.subject}
            </h2>
            {thread.trackersBlocked > 0 && (
              <span className="shrink-0 inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-amber-500/10 border border-amber-500/20 text-[11px] font-medium text-amber-300">
                <ShieldAlert className="w-3.5 h-3.5" />
                {thread.trackersBlocked}{' '}
                {thread.trackersBlocked === 1 ? 'tracker' : 'trackers'} blocked
              </span>
            )}
          </div>
        </div>

        {/* Local On-Device AI 2-Bullet TL;DR Card */}
        {summary && (
          <div className="p-3.5 rounded-lg bg-[#12161f] border border-emerald-500/25 space-y-2">
            <div className="flex items-center justify-between text-[11px]">
              <span className="font-medium text-emerald-300 flex items-center gap-1.5">
                <Cpu className="w-3.5 h-3.5" />
                On-Device Summary
              </span>
              <span className="text-[#667085] flex items-center gap-2">
                <span>Private · {summary.modelUsed}</span>
                {summary.actionRequired && (
                  <span className="px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300 font-medium">
                    Action Needed
                  </span>
                )}
              </span>
            </div>
            <ul className="space-y-1.5 pl-4 m-0 list-disc text-[12.5px] text-[#dce2ec] leading-relaxed selectable">
              {summary.bullets.map((b, i) => (
                <li key={i}>{b}</li>
              ))}
            </ul>
          </div>
        )}

        {aiError && (
          <div className="px-3.5 py-2.5 rounded-lg bg-rose-500/10 border border-rose-500/25 text-[12px] text-rose-200 font-mono">
            Local AI: {aiError}
          </div>
        )}

        {/* Messages Stack */}
        <div className="space-y-3">
          {messages.map((msg, idx) => (
            <MessageCard
              key={msg.id}
              message={msg}
              defaultExpanded={idx === messages.length - 1}
            />
          ))}
        </div>

        {/* Bottom Quick Reply Prompt */}
        <div className="pt-2">
          <button
            onClick={() => onOpenReply(detail)}
            className="w-full px-4 py-3 rounded-lg bg-[#11141c] hover:bg-[#151923] border border-white/[0.068] text-left flex items-center justify-between text-[12.5px] text-[#7c8599] hover:text-[#cbd5e1] transition-colors"
          >
            <span className="flex items-center gap-2">
              <Reply className="w-3.5 h-3.5" />
              <span>Reply to {thread.lastSenderName}…</span>
            </span>
            <span className="kbd">R</span>
          </button>
        </div>
      </div>
    </main>
  );
}
