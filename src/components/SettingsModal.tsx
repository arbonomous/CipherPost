import { useEffect, useState } from 'react';
import {
  CheckCircle2,
  Cpu,
  Database,
  KeyRound,
  Loader2,
  ShieldCheck,
  X,
  XCircle,
} from 'lucide-react';
import { checkLocalAi, saveSettings } from '../lib/tauri';
import type { AiHealthResponse, AppSettings, PrivacyStats } from '../types/mail';

interface SettingsModalProps {
  open: boolean;
  settings: AppSettings;
  privacyStats: PrivacyStats;
  onClose: () => void;
  onSettingsSaved: (updated: AppSettings) => void;
}

export function SettingsModal({
  open,
  settings,
  privacyStats,
  onClose,
  onSettingsSaved,
}: SettingsModalProps) {
  const [form, setForm] = useState<AppSettings>(settings);
  const [health, setHealth] = useState<AiHealthResponse | null>(null);
  const [checking, setChecking] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setForm(settings);
      setHealth(null);
      setError(null);
    }
  }, [open, settings]);

  if (!open) return null;

  const handlePingLocalAi = async () => {
    setChecking(true);
    setError(null);
    try {
      const res = await checkLocalAi(form.aiEndpoint);
      setHealth(res);
      if (res.availableModels.length > 0 && !res.availableModels.includes(form.aiModel)) {
        setForm((prev) => ({ ...prev, aiModel: res.availableModels[0] }));
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setChecking(false);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      const saved = await saveSettings(form);
      onSettingsSaved(saved);
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
        aria-label="Vault, Privacy Shield and Local AI Settings"
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-[600px] rounded-xl bg-[#11141c] border border-white/[0.12] shadow-2xl overflow-hidden"
      >
        <div className="px-5 py-4 border-b border-white/[0.07] flex items-center justify-between">
          <div>
            <h2 className="text-[14.5px] font-semibold text-[#f1f5f9]">
              Vault Security &amp; Local AI Triage
            </h2>
            <p className="text-[11.5px] text-[#7c8599]">
              Strict loopback policy · Zero cloud telemetry
            </p>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded-md text-[#667085] hover:text-white hover:bg-white/[0.06]"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 space-y-5 max-h-[75vh] overflow-y-auto">
          {/* Encrypted Vault Diagnostics */}
          <div className="p-3.5 rounded-lg bg-[#0d1017] border border-white/[0.07] space-y-2.5">
            <div className="text-[11px] font-mono uppercase tracking-wider text-emerald-400 flex items-center gap-1.5">
              <Database className="w-3.5 h-3.5" />
              Local Encrypted Storage Diagnostics
            </div>
            <div className="grid grid-cols-2 gap-2.5 text-[11.5px]">
              <div className="p-2.5 rounded bg-[#11141c] border border-white/[0.05]">
                <div className="text-[10px] font-mono text-[#667085]">AT-REST CIPHER</div>
                <div className="text-[#edf0f5] font-medium mt-0.5">
                  SQLCipher AES-256 ({privacyStats.sqlcipherActive ? 'Active' : 'Local'})
                </div>
              </div>
              <div className="p-2.5 rounded bg-[#11141c] border border-white/[0.05]">
                <div className="text-[10px] font-mono text-[#667085]">KEY STORAGE</div>
                <div className="text-[#edf0f5] font-medium mt-0.5 flex items-center gap-1">
                  <KeyRound className="w-3 h-3 text-emerald-400" />
                  <span className="truncate">{privacyStats.keychainBackend}</span>
                </div>
              </div>
              <div className="p-2.5 rounded bg-[#11141c] border border-white/[0.05]">
                <div className="text-[10px] font-mono text-[#667085]">FTS5 INDEXED</div>
                <div className="text-[#edf0f5] font-mono mt-0.5">
                  {privacyStats.totalMessagesIndexed.toLocaleString()} messages
                </div>
              </div>
              <div className="p-2.5 rounded bg-[#11141c] border border-white/[0.05]">
                <div className="text-[10px] font-mono text-[#667085]">TRACKERS NEUTRALIZED</div>
                <div className="text-amber-300 font-mono mt-0.5">
                  {privacyStats.totalTrackersBlocked.toLocaleString()} blocked
                </div>
              </div>
            </div>
          </div>

          {/* Local AI (Ollama / MLX) Configuration */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Cpu className="w-4 h-4 text-emerald-400" />
                <span className="text-[13px] font-semibold text-[#edf0f5]">
                  On-Device AI Triage (Ollama / MLX)
                </span>
              </div>
              <label className="flex items-center gap-2 text-[12px] text-[#9ba3b5] cursor-pointer">
                <input
                  type="checkbox"
                  checked={form.aiEnabled}
                  onChange={(e) => setForm({ ...form, aiEnabled: e.target.checked })}
                  className="accent-emerald-500"
                />
                Enabled
              </label>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <label className="space-y-1">
                <span className="text-[10.5px] font-mono uppercase text-[#7c8599]">
                  Loopback Endpoint (127.0.0.1 / localhost only)
                </span>
                <input
                  id="input-ai-endpoint"
                  type="text"
                  value={form.aiEndpoint}
                  onChange={(e) => setForm({ ...form, aiEndpoint: e.target.value })}
                  className="w-full h-9 px-3 rounded-md bg-[#0b0d13] border border-white/[0.09] font-mono text-[12px] text-[#edf0f5] focus:border-emerald-500/50 focus:outline-none"
                />
              </label>
              <label className="space-y-1">
                <span className="text-[10.5px] font-mono uppercase text-[#7c8599]">
                  Local Model ID
                </span>
                <input
                  id="input-ai-model"
                  type="text"
                  value={form.aiModel}
                  onChange={(e) => setForm({ ...form, aiModel: e.target.value })}
                  placeholder="llama3.2:latest"
                  className="w-full h-9 px-3 rounded-md bg-[#0b0d13] border border-white/[0.09] font-mono text-[12px] text-[#edf0f5] focus:border-emerald-500/50 focus:outline-none"
                />
              </label>
            </div>

            <div className="flex items-center justify-between gap-2">
              <button
                type="button"
                onClick={handlePingLocalAi}
                disabled={checking}
                className="h-7 px-3 rounded-md bg-white/[0.05] hover:bg-white/[0.09] border border-white/[0.09] text-[11.5px] text-[#dce2ec] flex items-center gap-1.5"
              >
                {checking && <Loader2 className="w-3 h-3 animate-spin" />}
                <span>Test Local Endpoint</span>
              </button>
              <span className="text-[10.5px] font-mono text-[#596174]">
                External cloud URLs are rejected by Rust backend
              </span>
            </div>

            {health && (
              <div className="p-2.5 rounded-md bg-[#0b0d13] border border-white/[0.08] text-[11.5px] space-y-1">
                <div className="flex items-center justify-between font-mono">
                  <span className="flex items-center gap-1.5">
                    {health.reachable ? (
                      <>
                        <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
                        <span className="text-emerald-300">Connected ({health.latencyMs}ms)</span>
                      </>
                    ) : (
                      <>
                        <XCircle className="w-3.5 h-3.5 text-amber-400" />
                        <span className="text-amber-300">Server Not Running Locally</span>
                      </>
                    )}
                  </span>
                </div>
                {health.error && <p className="text-[11px] text-[#7c8599] m-0">{health.error}</p>}
                {health.availableModels.length > 0 && (
                  <div className="flex items-center gap-1.5 flex-wrap pt-1">
                    {health.availableModels.slice(0, 6).map((m) => (
                      <button
                        key={m}
                        onClick={() => setForm({ ...form, aiModel: m })}
                        className={`px-2 py-0.5 rounded font-mono text-[10px] border ${
                          form.aiModel === m
                            ? 'bg-emerald-500/15 border-emerald-500/30 text-emerald-300'
                            : 'bg-white/[0.03] border-white/[0.08] text-[#8d96a8]'
                        }`}
                      >
                        {m}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Privacy Shield Toggles */}
          <div className="space-y-2 pt-1 border-t border-white/[0.06]">
            <div className="text-[11px] font-mono uppercase tracking-wider text-[#7c8599] flex items-center gap-1.5">
              <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
              Active Privacy &amp; Tracker Shield
            </div>
            {(
              [
                ['stripTrackers', 'Strip 1x1 spy pixels, tracker domains & remote CSS beacons in Rust'],
                ['blockRemoteImages', 'Block external images by default (one-click opt-in per message)'],
                ['autoVerifyPgp', 'Auto-verify PGP/MIME & inline OpenPGP signatures via Sequoia'],
              ] as const
            ).map(([field, label]) => (
              <label
                key={field}
                className="flex items-center justify-between py-1.5 text-[12.5px] text-[#cbd5e1] cursor-pointer"
              >
                <span>{label}</span>
                <input
                  type="checkbox"
                  checked={form[field]}
                  onChange={(e) => setForm({ ...form, [field]: e.target.checked })}
                  className="accent-emerald-500"
                />
              </label>
            ))}
          </div>

          {error && (
            <div className="px-3 py-2 rounded-md bg-rose-500/10 border border-rose-500/25 text-[11.5px] font-mono text-rose-200">
              {error}
            </div>
          )}
        </div>

        <div className="px-5 py-3.5 border-t border-white/[0.07] bg-[#0d1017] flex justify-end gap-2">
          <button
            onClick={onClose}
            className="h-8 px-3 rounded-md text-[#8d96a8] hover:text-white text-[12px]"
          >
            Cancel
          </button>
          <button
            id="btn-save-settings"
            onClick={handleSave}
            disabled={saving}
            className="h-8 px-4 rounded-md bg-[#edf0f5] hover:bg-white text-[#090a0d] text-[12px] font-semibold"
          >
            {saving ? 'Saving…' : 'Save Configuration'}
          </button>
        </div>
      </div>
    </div>
  );
}
