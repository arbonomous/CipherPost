import { useEffect, useState } from 'react';
import { Loader2, Lock, Send, ShieldCheck, Sparkles, X } from 'lucide-react';
import { aiDraftReply, sendMessage } from '../lib/tauri';
import type { Account, ThreadDetail } from '../types/mail';

interface ComposerModalProps {
  open: boolean;
  accounts: Account[];
  defaultAccountId: string | null;
  replyTarget: ThreadDetail | null;
  initialDraft?: string;
  aiEnabled: boolean;
  onClose: () => void;
  onSent: () => void;
}

export function ComposerModal({
  open,
  accounts,
  defaultAccountId,
  replyTarget,
  initialDraft,
  aiEnabled,
  onClose,
  onSent,
}: ComposerModalProps) {
  const [accountId, setAccountId] = useState(defaultAccountId || accounts[0]?.id || '');
  const [to, setTo] = useState('');
  const [cc, setCc] = useState('');
  const [subject, setSubject] = useState('');
  const [body, setBody] = useState('');
  const [pgpSign, setPgpSign] = useState(true);
  const [pgpEncrypt, setPgpEncrypt] = useState(false);
  const [sending, setSending] = useState(false);
  const [drafting, setDrafting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    if (replyTarget) {
      const last = replyTarget.messages[replyTarget.messages.length - 1];
      setAccountId(replyTarget.thread.accountId || defaultAccountId || accounts[0]?.id || '');
      setTo(last?.senderEmail || replyTarget.thread.lastSenderEmail);
      setCc('');
      const subj = replyTarget.thread.subject;
      setSubject(subj.toLowerCase().startsWith('re:') ? subj : `Re: ${subj}`);
      setBody(initialDraft || '');
    } else {
      setAccountId(defaultAccountId || accounts[0]?.id || '');
      setTo('');
      setCc('');
      setSubject('');
      setBody(initialDraft || '');
    }
  }, [open, replyTarget, initialDraft, defaultAccountId, accounts]);

  if (!open) return null;

  const handleSend = async () => {
    const recipients = to
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);
    if (recipients.length === 0) {
      setError('Specify at least one recipient email address.');
      return;
    }

    setSending(true);
    setError(null);
    try {
      const lastMsg = replyTarget?.messages[replyTarget.messages.length - 1];
      await sendMessage({
        accountId: accountId || accounts[0]?.id,
        to: recipients,
        cc: cc
          .split(',')
          .map((s) => s.trim())
          .filter(Boolean),
        bcc: [],
        subject: subject.trim() || '(No Subject)',
        bodyText: body,
        inReplyTo: lastMsg?.messageIdHeader || null,
        threadId: replyTarget?.thread.id || null,
        pgpSign,
        pgpEncrypt,
      });
      onSent();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSending(false);
    }
  };

  const handleLocalAiDraft = async () => {
    if (!replyTarget) return;
    setDrafting(true);
    setError(null);
    try {
      const res = await aiDraftReply(replyTarget.thread.id, body.trim() || undefined);
      setBody(res.draftText);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setDrafting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 bg-black/70 backdrop-blur-[2px] flex items-center justify-center p-4"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-label={replyTarget ? 'Reply to Thread' : 'Compose New Message'}
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-[660px] rounded-xl bg-[#11141c] border border-white/[0.12] shadow-2xl overflow-hidden flex flex-col"
      >
        {/* Header */}
        <div className="px-4 py-3 border-b border-white/[0.07] flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="text-[13.5px] font-semibold text-[#f1f5f9]">
              {replyTarget ? 'Reply' : 'New Message'}
            </span>
            <span className="text-[10.5px] font-mono text-emerald-400/90 px-1.5 py-0.5 rounded bg-emerald-500/10">
              DIRECT SMTP
            </span>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded text-[#667085] hover:text-white hover:bg-white/[0.06]"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Fields */}
        <div className="divide-y divide-white/[0.055] text-[12.5px]">
          <div className="px-4 py-2 flex items-center gap-3">
            <span className="w-14 text-[11px] font-mono text-[#667085]">FROM</span>
            <select
              value={accountId}
              onChange={(e) => setAccountId(e.target.value)}
              className="flex-1 bg-transparent text-[#edf0f5] focus:outline-none"
            >
              {accounts.map((a) => (
                <option key={a.id} value={a.id} className="bg-[#11141c]">
                  {a.displayName} &lt;{a.email}&gt;
                </option>
              ))}
            </select>
          </div>
          <div className="px-4 py-2 flex items-center gap-3">
            <span className="w-14 text-[11px] font-mono text-[#667085]">TO</span>
            <input
              type="text"
              value={to}
              onChange={(e) => setTo(e.target.value)}
              placeholder="recipient@domain.com"
              className="flex-1 bg-transparent text-[#edf0f5] placeholder-[#4a5264] focus:outline-none"
            />
          </div>
          <div className="px-4 py-2 flex items-center gap-3">
            <span className="w-14 text-[11px] font-mono text-[#667085]">CC</span>
            <input
              type="text"
              value={cc}
              onChange={(e) => setCc(e.target.value)}
              placeholder="Optional comma-separated addresses"
              className="flex-1 bg-transparent text-[#edf0f5] placeholder-[#4a5264] focus:outline-none"
            />
          </div>
          <div className="px-4 py-2 flex items-center gap-3">
            <span className="w-14 text-[11px] font-mono text-[#667085]">SUBJECT</span>
            <input
              type="text"
              value={subject}
              onChange={(e) => setSubject(e.target.value)}
              placeholder="Subject line"
              className="flex-1 bg-transparent text-[#f1f5f9] font-medium placeholder-[#4a5264] focus:outline-none"
            />
          </div>
        </div>

        {/* Body */}
        <div className="p-4 border-t border-white/[0.055]">
          <textarea
            value={body}
            onChange={(e) => setBody(e.target.value)}
            rows={10}
            placeholder="Write your message… (plain-text zero-tracker outbound delivery)"
            className="w-full bg-transparent text-[13px] text-[#dce2ec] placeholder-[#4a5264] leading-relaxed focus:outline-none resize-none selectable"
          />
          {error && (
            <div className="mt-2 px-3 py-1.5 rounded bg-rose-500/10 border border-rose-500/25 text-[11.5px] font-mono text-rose-200">
              {error}
            </div>
          )}
        </div>

        {/* Footer Controls */}
        <div className="px-4 py-3 border-t border-white/[0.07] bg-[#0d1017] flex items-center justify-between gap-2 flex-wrap">
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setPgpSign((v) => !v)}
              className={`h-7 px-2.5 rounded-md border text-[11px] font-mono flex items-center gap-1.5 transition-colors ${
                pgpSign
                  ? 'bg-emerald-500/12 border-emerald-500/30 text-emerald-300'
                  : 'bg-white/[0.03] border-white/[0.07] text-[#667085]'
              }`}
            >
              <ShieldCheck className="w-3.5 h-3.5" />
              PGP Sign
            </button>

            <button
              type="button"
              onClick={() => setPgpEncrypt((v) => !v)}
              className={`h-7 px-2.5 rounded-md border text-[11px] font-mono flex items-center gap-1.5 transition-colors ${
                pgpEncrypt
                  ? 'bg-emerald-500/12 border-emerald-500/30 text-emerald-300'
                  : 'bg-white/[0.03] border-white/[0.07] text-[#667085]'
              }`}
            >
              <Lock className="w-3.5 h-3.5" />
              PGP Encrypt
            </button>

            {aiEnabled && replyTarget && (
              <button
                type="button"
                onClick={handleLocalAiDraft}
                disabled={drafting}
                className="h-7 px-2.5 rounded-md bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] text-[11px] text-[#cbd5e1] flex items-center gap-1.5"
              >
                {drafting ? (
                  <Loader2 className="w-3 h-3 animate-spin" />
                ) : (
                  <Sparkles className="w-3 h-3 text-amber-300" />
                )}
                <span>Local AI Draft</span>
              </button>
            )}
          </div>

          <button
            id="btn-send-email"
            onClick={handleSend}
            disabled={sending}
            className="h-8 px-4 rounded-md bg-[#edf0f5] hover:bg-white text-[#090a0d] text-[12.5px] font-semibold flex items-center gap-2 transition-colors disabled:opacity-50"
          >
            {sending ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Send className="w-3.5 h-3.5" />}
            <span>{sending ? 'Delivering…' : 'Send Directly'}</span>
          </button>
        </div>
      </div>
    </div>
  );
}
