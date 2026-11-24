import { useState } from 'react';
import {
  CheckCircle2,
  ExternalLink,
  KeyRound,
  Loader2,
  Lock,
  Server,
  ShieldCheck,
  X,
  XCircle,
} from 'lucide-react';
import {
  addAccount,
  openExternalUrl,
  startGmailOAuth,
  testAccountConnection,
} from '../lib/tauri';
import type {
  Account,
  AuthType,
  ConnectionTestResult,
  NewAccountInput,
  ProviderKind,
  TlsMode,
} from '../types/mail';

interface AccountSetupModalProps {
  open: boolean;
  onClose: () => void;
  onAccountAdded: (account: Account) => void;
}

interface ProviderPreset {
  kind: ProviderKind;
  label: string;
  tagline: string;
  authType: AuthType;
  imapHost: string;
  imapPort: number;
  imapTls: TlsMode;
  smtpHost: string;
  smtpPort: number;
  smtpTls: TlsMode;
  secretLabel: string;
  secretPlaceholder: string;
  accentColor: string;
  hint: string;
}

const PRESETS: ProviderPreset[] = [
  {
    kind: 'gmail',
    label: 'Gmail',
    tagline: 'Google App Password or OAuth2',
    authType: 'password',
    imapHost: 'imap.gmail.com',
    imapPort: 993,
    imapTls: 'implicit',
    smtpHost: 'smtp.gmail.com',
    smtpPort: 465,
    smtpTls: 'implicit',
    secretLabel: 'Google App Password (16 characters)',
    secretPlaceholder: 'xxxx xxxx xxxx xxxx',
    accentColor: '#ef4444',
    hint: 'In Google Account → Security → 2-Step Verification → App passwords, generate a password for CipherPost. (Or switch Auth Method to OAuth2 if using an access token.)',
  },
  {
    kind: 'icloud',
    label: 'iCloud Mail',
    tagline: 'Apple App-Specific Password · TLS 1.3',
    authType: 'password',
    imapHost: 'imap.mail.me.com',
    imapPort: 993,
    imapTls: 'implicit',
    smtpHost: 'smtp.mail.me.com',
    smtpPort: 587,
    smtpTls: 'starttls',
    secretLabel: 'iCloud App-Specific Password',
    secretPlaceholder: 'xxxx-xxxx-xxxx-xxxx',
    accentColor: '#38bdf8',
    hint: 'Generate an App-Specific Password in appleid.apple.com → Sign-In & Security.',
  },
  {
    kind: 'proton_bridge',
    label: 'Proton Bridge',
    tagline: 'Localhost Loopback · 127.0.0.1:1143',
    authType: 'password',
    imapHost: '127.0.0.1',
    imapPort: 1143,
    imapTls: 'starttls',
    smtpHost: '127.0.0.1',
    smtpPort: 1025,
    smtpTls: 'starttls',
    secretLabel: 'Proton Bridge Mailbox Password',
    secretPlaceholder: 'Paste Bridge-generated password',
    accentColor: '#10b981',
    hint: 'Connects strictly over loopback 127.0.0.1 with Proton Mail Bridge local STARTTLS encryption.',
  },
  {
    kind: 'custom',
    label: 'Custom IMAP',
    tagline: 'Fastmail, Migadu, Self-Hosted Dovecot',
    authType: 'password',
    imapHost: 'imap.fastmail.com',
    imapPort: 993,
    imapTls: 'implicit',
    smtpHost: 'smtp.fastmail.com',
    smtpPort: 465,
    smtpTls: 'implicit',
    secretLabel: 'IMAP/SMTP Password',
    secretPlaceholder: 'Account or app password',
    accentColor: '#f59e0b',
    hint: 'Supports implicit TLS (993/465) and STARTTLS (143/587) with direct TLS 1.3 certificate validation.',
  },
];

export function AccountSetupModal({
  open,
  onClose,
  onAccountAdded,
}: AccountSetupModalProps) {
  const [preset, setPreset] = useState<ProviderPreset>(PRESETS[2]);
  const [email, setEmail] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [secret, setSecret] = useState('');
  const [authType, setAuthType] = useState<AuthType>(PRESETS[2].authType);
  const [imapHost, setImapHost] = useState(PRESETS[2].imapHost);
  const [imapPort, setImapPort] = useState(PRESETS[2].imapPort);
  const [imapTls, setImapTls] = useState<TlsMode>(PRESETS[2].imapTls);
  const [smtpHost, setSmtpHost] = useState(PRESETS[2].smtpHost);
  const [smtpPort, setSmtpPort] = useState(PRESETS[2].smtpPort);
  const [smtpTls, setSmtpTls] = useState<TlsMode>(PRESETS[2].smtpTls);

  const [testing, setTesting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [oauthRunning, setOauthRunning] = useState(false);
  const [showOAuthConfig, setShowOAuthConfig] = useState(false);
  const [oauthClientId, setOauthClientId] = useState(
    () => localStorage.getItem('cp_google_client_id') || '',
  );
  const [oauthClientSecret, setOauthClientSecret] = useState(
    () => localStorage.getItem('cp_google_client_secret') || '',
  );
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [testResult, setTestResult] = useState<ConnectionTestResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  const handleGoogleSignIn = async () => {
    const cid = oauthClientId.trim();
    if (!cid) {
      setShowOAuthConfig(true);
      setError('Enter your Google OAuth Desktop Client ID below, or click "Get App Password ↗" for instant setup.');
      return;
    }
    localStorage.setItem('cp_google_client_id', cid);
    if (oauthClientSecret.trim()) {
      localStorage.setItem('cp_google_client_secret', oauthClientSecret.trim());
    }
    setError(null);
    setOauthRunning(true);
    try {
      const res = await startGmailOAuth(cid, oauthClientSecret.trim() || undefined);
      setEmail(res.email);
      setDisplayName(res.displayName);
      setSecret(res.accessToken);
      setAuthType('oauth2');
      const created = await addAccount({
        email: res.email,
        displayName: res.displayName,
        provider: 'gmail',
        authType: 'oauth2',
        imapHost: 'imap.gmail.com',
        imapPort: 993,
        imapTls: 'implicit',
        smtpHost: 'smtp.gmail.com',
        smtpPort: 465,
        smtpTls: 'implicit',
        secret: res.accessToken,
        accentColor: '#ef4444',
      });
      onAccountAdded(created);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setOauthRunning(false);
    }
  };

  const applyPreset = (p: ProviderPreset) => {
    setPreset(p);
    setAuthType(p.authType);
    setImapHost(p.imapHost);
    setImapPort(p.imapPort);
    setImapTls(p.imapTls);
    setSmtpHost(p.smtpHost);
    setSmtpPort(p.smtpPort);
    setSmtpTls(p.smtpTls);
    setShowAdvanced(p.kind === 'custom');
    setTestResult(null);
    setError(null);
  };

  const buildPayload = (): NewAccountInput => {
    const trimmedSecret = secret.trim();
    const looksLikeOAuthToken = trimmedSecret.startsWith('ya29.');
    const effectiveAuthType: AuthType =
      preset.kind === 'gmail' && looksLikeOAuthToken ? 'oauth2' : authType;
    const normalizedSecret =
      preset.kind === 'gmail' && effectiveAuthType === 'password'
        ? trimmedSecret.replace(/\s+/g, '')
        : trimmedSecret;

    return {
      email: email.trim(),
      displayName: displayName.trim() || email.trim().split('@')[0] || 'CipherPost User',
      provider: preset.kind,
      authType: effectiveAuthType,
      imapHost: imapHost.trim(),
      imapPort: Number(imapPort),
      imapTls,
      smtpHost: smtpHost.trim(),
      smtpPort: Number(smtpPort),
      smtpTls,
      secret: normalizedSecret,
      accentColor: preset.accentColor,
    };
  };

  const handleTestHandshake = async () => {
    setError(null);
    setTesting(true);
    setTestResult(null);
    try {
      const res = await testAccountConnection(buildPayload());
      setTestResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    if (!email.includes('@')) {
      setError('Enter a valid email address.');
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const created = await addAccount(buildPayload());
      onAccountAdded(created);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 bg-black/75 backdrop-blur-[2px] flex items-center justify-center p-4"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-label="Connect Your Email"
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-[640px] rounded-xl bg-[#11141c] border border-white/[0.12] shadow-2xl overflow-hidden flex flex-col max-h-[90vh]"
      >
        {/* Modal Header */}
        <div className="px-5 py-4 border-b border-white/[0.07] flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-emerald-500/10 border border-emerald-500/25 flex items-center justify-center">
              <Server className="w-4 h-4 text-emerald-400" />
            </div>
            <div>
              <h2 className="text-[14.5px] font-semibold text-[#f1f5f9]">
                Connect Your Email
              </h2>
              <p className="text-[11.5px] text-[#7c8599]">
                Direct connection from your Mac · Saved securely in macOS Keychain
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded-md text-[#667085] hover:text-white hover:bg-white/[0.06]"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 overflow-y-auto space-y-4">
          {/* One-Click Provider Presets */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
            {PRESETS.map((p) => {
              const active = preset.kind === p.kind;
              return (
                <button
                  key={p.kind}
                  id={`preset-${p.kind}`}
                  onClick={() => applyPreset(p)}
                  className={`p-2.5 rounded-lg border text-left transition-all ${
                    active
                      ? 'bg-white/[0.07] border-white/[0.22]'
                      : 'bg-[#0d1017] border-white/[0.06] hover:border-white/[0.13]'
                  }`}
                >
                  <div className="flex items-center gap-2">
                    <span
                      className="w-2 h-2 rounded-full"
                      style={{ backgroundColor: p.accentColor }}
                    />
                    <span className="text-[12.5px] font-semibold text-[#edf0f5]">{p.label}</span>
                  </div>
                  <div className="text-[10px] text-[#667085] mt-1 leading-snug">{p.tagline}</div>
                </button>
              );
            })}
          </div>

          <div className="px-3 py-2.5 rounded-md bg-[#0d1017] border border-white/[0.06] text-[11.5px] text-[#8d96a8] flex items-center justify-between gap-3 flex-wrap">
            <div className="flex items-center gap-2 min-w-0 flex-1">
              <KeyRound className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
              <span>{preset.hint}</span>
            </div>
            {preset.kind === 'gmail' && (
              <button
                type="button"
                onClick={() => openExternalUrl('https://myaccount.google.com/apppasswords')}
                className="h-7 px-2.5 rounded bg-white/[0.06] hover:bg-white/[0.11] text-[#edf0f5] text-[11px] font-medium flex items-center gap-1.5 shrink-0 transition-colors"
              >
                <span>Open App Passwords</span>
                <ExternalLink className="w-3 h-3" />
              </button>
            )}
            {preset.kind === 'icloud' && (
              <button
                type="button"
                onClick={() => openExternalUrl('https://appleid.apple.com')}
                className="h-7 px-2.5 rounded bg-white/[0.06] hover:bg-white/[0.11] text-[#edf0f5] text-[11px] font-medium flex items-center gap-1.5 shrink-0 transition-colors"
              >
                <span>Open Apple ID</span>
                <ExternalLink className="w-3 h-3" />
              </button>
            )}
          </div>

          {preset.kind === 'gmail' && (
            <div className="p-3 rounded-lg bg-[#0d1017] border border-white/[0.08] space-y-2.5">
              <div className="flex items-center justify-between gap-2 flex-wrap">
                <button
                  type="button"
                  onClick={handleGoogleSignIn}
                  disabled={oauthRunning}
                  className="h-9 px-4 rounded-md bg-white hover:bg-[#f1f5f9] text-[#090a0d] text-[12.5px] font-semibold flex items-center gap-2 transition-colors disabled:opacity-50"
                >
                  {oauthRunning ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <span className="font-bold text-[#ea4335]">G</span>
                  )}
                  <span>{oauthRunning ? 'Waiting for Google in browser…' : 'Sign in with Google'}</span>
                </button>
                <button
                  type="button"
                  onClick={() => setShowOAuthConfig((v) => !v)}
                  className="text-[11px] text-[#7c8599] hover:text-[#edf0f5]"
                >
                  {showOAuthConfig ? 'Hide OAuth Client ID' : 'Configure Google OAuth Client ID'}
                </button>
              </div>
              {showOAuthConfig && (
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 pt-1">
                  <input
                    type="text"
                    value={oauthClientId}
                    onChange={(e) => setOauthClientId(e.target.value)}
                    placeholder="Google Desktop Client ID (xxxx.apps.googleusercontent.com)"
                    className="h-8 px-2.5 rounded bg-[#090b10] border border-white/[0.08] text-[11.5px] font-mono text-[#dce2ec]"
                  />
                  <input
                    type="password"
                    value={oauthClientSecret}
                    onChange={(e) => setOauthClientSecret(e.target.value)}
                    placeholder="Client Secret (optional for Desktop)"
                    className="h-8 px-2.5 rounded bg-[#090b10] border border-white/[0.08] text-[11.5px] font-mono text-[#dce2ec]"
                  />
                </div>
              )}
            </div>
          )}

          {/* Identity & Secret */}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <label className="space-y-1">
              <span className="text-[11px] font-mono uppercase text-[#7c8599]">Email Address</span>
              <input
                id="input-account-email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@domain.com"
                className="w-full h-9 px-3 rounded-md bg-[#0b0d13] border border-white/[0.09] focus:border-emerald-500/50 focus:outline-none text-[12.5px] text-[#edf0f5]"
              />
            </label>
            <label className="space-y-1">
              <span className="text-[11px] font-mono uppercase text-[#7c8599]">Display Name</span>
              <input
                type="text"
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
                placeholder="Elena Vance"
                className="w-full h-9 px-3 rounded-md bg-[#0b0d13] border border-white/[0.09] focus:border-emerald-500/50 focus:outline-none text-[12.5px] text-[#edf0f5]"
              />
            </label>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <label className="sm:col-span-2 space-y-1">
              <span className="text-[11px] font-mono uppercase text-[#7c8599]">
                {preset.secretLabel}
              </span>
              <input
                id="input-account-secret"
                type="password"
                value={secret}
                onChange={(e) => setSecret(e.target.value)}
                placeholder={preset.secretPlaceholder}
                className="w-full h-9 px-3 rounded-md bg-[#0b0d13] border border-white/[0.09] focus:border-emerald-500/50 focus:outline-none text-[12.5px] font-mono text-[#edf0f5]"
              />
            </label>
            <label className="space-y-1">
              <span className="text-[11px] font-mono uppercase text-[#7c8599]">Auth Method</span>
              <select
                value={authType}
                onChange={(e) => setAuthType(e.target.value as AuthType)}
                className="w-full h-9 px-2.5 rounded-md bg-[#0b0d13] border border-white/[0.09] text-[12.5px] text-[#edf0f5] focus:outline-none"
              >
                <option value="password">App / Bridge Password</option>
                <option value="oauth2">OAuth2 (XOAUTH2)</option>
              </select>
            </label>
          </div>

          {/* Collapsible Advanced IMAP & SMTP Host / Port / TLS Controls */}
          <div className="pt-1">
            <button
              type="button"
              onClick={() => setShowAdvanced((v) => !v)}
              className="text-[11.5px] text-[#7c8599] hover:text-[#edf0f5] flex items-center gap-1.5 transition-colors"
            >
              <span>{showAdvanced ? '▾' : '▸'}</span>
              <span>Advanced server settings ({imapHost}:{imapPort})</span>
            </button>

            {showAdvanced && (
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mt-2.5">
                <div className="p-3 rounded-lg bg-[#0d1017] border border-white/[0.06] space-y-2">
                  <div className="text-[10.5px] font-medium uppercase text-[#7c8599] flex items-center gap-1.5">
                    <Lock className="w-3 h-3 text-emerald-400" />
                    Incoming Mail (IMAP)
                  </div>
                  <input
                    type="text"
                    value={imapHost}
                    onChange={(e) => setImapHost(e.target.value)}
                    className="w-full h-8 px-2.5 rounded bg-[#090b10] border border-white/[0.08] text-[12px] font-mono text-[#dce2ec]"
                  />
                  <div className="flex items-center gap-2">
                    <input
                      type="number"
                      value={imapPort}
                      onChange={(e) => setImapPort(Number(e.target.value))}
                      className="w-24 h-8 px-2.5 rounded bg-[#090b10] border border-white/[0.08] text-[12px] font-mono text-[#dce2ec]"
                    />
                    <select
                      value={imapTls}
                      onChange={(e) => setImapTls(e.target.value as TlsMode)}
                      className="flex-1 h-8 px-2 rounded bg-[#090b10] border border-white/[0.08] text-[12px] text-[#dce2ec]"
                    >
                      <option value="implicit">Implicit TLS (993)</option>
                      <option value="starttls">STARTTLS (143 / 1143)</option>
                    </select>
                  </div>
                </div>

                <div className="p-3 rounded-lg bg-[#0d1017] border border-white/[0.06] space-y-2">
                  <div className="text-[10.5px] font-medium uppercase text-[#7c8599] flex items-center gap-1.5">
                    <Lock className="w-3 h-3 text-emerald-400" />
                    Outgoing Mail (SMTP)
                  </div>
                  <input
                    type="text"
                    value={smtpHost}
                    onChange={(e) => setSmtpHost(e.target.value)}
                    className="w-full h-8 px-2.5 rounded bg-[#090b10] border border-white/[0.08] text-[12px] font-mono text-[#dce2ec]"
                  />
                  <div className="flex items-center gap-2">
                    <input
                      type="number"
                      value={smtpPort}
                      onChange={(e) => setSmtpPort(Number(e.target.value))}
                      className="w-24 h-8 px-2.5 rounded bg-[#090b10] border border-white/[0.08] text-[12px] font-mono text-[#dce2ec]"
                    />
                    <select
                      value={smtpTls}
                      onChange={(e) => setSmtpTls(e.target.value as TlsMode)}
                      className="flex-1 h-8 px-2 rounded bg-[#090b10] border border-white/[0.08] text-[12px] text-[#dce2ec]"
                    >
                      <option value="implicit">Implicit TLS (465)</option>
                      <option value="starttls">STARTTLS (587 / 1025)</option>
                    </select>
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* Live TLS Handshake & Capability Diagnostic Output */}
          {testResult && (
            <div className="p-3.5 rounded-lg bg-[#0b0d13] border border-white/[0.09] space-y-2">
              <div className="flex items-center justify-between text-[11.5px]">
                <span className="font-mono font-semibold flex items-center gap-1.5">
                  {testResult.success ? (
                    <>
                      <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                      <span className="text-emerald-300">Live TLS &amp; Auth Handshake Passed</span>
                    </>
                  ) : (
                    <>
                      <XCircle className="w-4 h-4 text-rose-400" />
                      <span className="text-rose-300">Handshake Verification Failed</span>
                    </>
                  )}
                </span>
                {testResult.imapTlsVersion && (
                  <span className="text-[10px] font-mono text-[#7c8599]">
                    {testResult.imapTlsVersion}
                  </span>
                )}
              </div>

              <div className="space-y-1.5">
                {testResult.steps.map((s, idx) => (
                  <div
                    key={idx}
                    className="flex items-start justify-between gap-2 text-[11px] font-mono py-1 border-t border-white/[0.04]"
                  >
                    <div className="flex items-start gap-2 min-w-0">
                      <span
                        className={`w-1.5 h-1.5 rounded-full shrink-0 mt-1.5 ${
                          s.status === 'passed' ? 'bg-emerald-400' : 'bg-rose-400'
                        }`}
                      />
                      <span className="text-[#dce2ec] shrink-0">{s.step}</span>
                      <span className="text-[#8d96a8] break-words min-w-0">{s.detail}</span>
                    </div>
                    <span className="text-[#596174] tabular-nums shrink-0">{s.latencyMs}ms</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {error && (
            <div className="px-3 py-2 rounded-md bg-rose-500/10 border border-rose-500/25 text-[12px] text-rose-200 font-mono">
              {error}
            </div>
          )}
        </div>

        {/* Footer Buttons */}
        <div className="px-5 py-3.5 border-t border-white/[0.07] bg-[#0d1017] flex items-center justify-between gap-3">
          <div className="flex items-center gap-1.5 text-[11px] text-[#667085]">
            <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
            <span>Stored exclusively in OS Keychain</span>
          </div>

          <div className="flex items-center gap-2">
            <button
              id="btn-test-tls-handshake"
              onClick={handleTestHandshake}
              disabled={testing}
              className="h-8 px-3 rounded-md bg-white/[0.05] hover:bg-white/[0.09] border border-white/[0.1] text-[#dce2ec] text-[12px] font-medium flex items-center gap-1.5 transition-colors disabled:opacity-50"
            >
              {testing && <Loader2 className="w-3.5 h-3.5 animate-spin text-emerald-400" />}
              <span>{testing ? 'Testing TLS…' : 'Test Connection & TLS'}</span>
            </button>

            <button
              id="btn-save-account"
              onClick={handleSave}
              disabled={saving}
              className="h-8 px-3.5 rounded-md bg-[#edf0f5] hover:bg-white text-[#090a0d] text-[12px] font-semibold flex items-center gap-1.5 transition-colors disabled:opacity-50"
            >
              {saving ? 'Sealing…' : 'Add Account'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
