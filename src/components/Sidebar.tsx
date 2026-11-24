import {
  Archive,
  Clock,
  FileEdit,
  Inbox,
  KeyRound,
  PanelLeftClose,
  PanelLeftOpen,
  PenSquare,
  Plus,
  RefreshCw,
  Search,
  Send,
  Settings,
  ShieldCheck,
  Trash2,
} from 'lucide-react';
import type { Account, FolderCounts, FolderRole, PrivacyStats } from '../types/mail';

interface SidebarProps {
  collapsed: boolean;
  onToggleCollapse: () => void;
  accounts: Account[];
  activeAccountId: string | null;
  onSelectAccount: (accountId: string | null) => void;
  activeFolder: FolderRole;
  onSelectFolder: (folder: FolderRole) => void;
  counts: FolderCounts;
  privacyStats: PrivacyStats;
  syncing: boolean;
  onSyncAll: () => void;
  onCompose: () => void;
  onOpenCommandPalette: () => void;
  onOpenAccountSetup: () => void;
  onOpenSettings: () => void;
}

const FOLDERS: Array<{
  role: FolderRole;
  label: string;
  shortcut: string;
  icon: typeof Inbox;
}> = [
  { role: 'inbox', label: 'Unified Inbox', shortcut: 'G I', icon: Inbox },
  { role: 'snoozed', label: 'Snoozed', shortcut: 'G Z', icon: Clock },
  { role: 'sent', label: 'Sent', shortcut: 'G S', icon: Send },
  { role: 'drafts', label: 'Drafts', shortcut: 'G D', icon: FileEdit },
  { role: 'archive', label: 'Vault / Archive', shortcut: 'G V', icon: Archive },
  { role: 'trash', label: 'Trash', shortcut: 'G T', icon: Trash2 },
];

export function Sidebar({
  collapsed,
  onToggleCollapse,
  accounts,
  activeAccountId,
  onSelectAccount,
  activeFolder,
  onSelectFolder,
  counts,
  privacyStats,
  syncing,
  onSyncAll,
  onCompose,
  onOpenCommandPalette,
  onOpenAccountSetup,
  onOpenSettings,
}: SidebarProps) {
  const getFolderCount = (role: FolderRole): number => {
    switch (role) {
      case 'inbox':
        return counts.inboxUnread;
      case 'snoozed':
        return counts.snoozedTotal;
      case 'sent':
        return counts.sentTotal;
      case 'drafts':
        return counts.draftsTotal;
      case 'archive':
        return counts.archiveTotal;
      case 'trash':
        return counts.trashTotal;
    }
  };

  return (
    <aside
      aria-label="Mailboxes and Accounts"
      className={`h-full flex flex-col bg-[#0d0f14] border-r border-white/[0.068] shrink-0 transition-[width] duration-150 ${
        collapsed ? 'w-[60px]' : 'w-[244px]'
      }`}
    >
      {/* Top macOS Overlay Titlebar & Brand Header */}
      <div
        data-tauri-drag-region
        className={`titlebar-drag h-12 flex items-center border-b border-white/[0.055] ${
          collapsed ? 'px-2 justify-center' : 'pl-[78px] pr-2.5 justify-between'
        }`}
      >
        {!collapsed && (
          <div data-tauri-drag-region className="flex items-center gap-2 min-w-0">
            <div
              data-tauri-drag-region
              className="w-5 h-5 rounded bg-gradient-to-b from-[#1e2430] to-[#12151c] border border-white/[0.12] flex items-center justify-center shrink-0"
            >
              <ShieldCheck className="w-3 h-3 text-emerald-400 pointer-events-none" strokeWidth={2.2} />
            </div>
            <span
              data-tauri-drag-region
              className="text-[12.5px] font-semibold tracking-[-0.02em] text-[#f1f5f9] truncate"
            >
              CipherPost
            </span>
          </div>
        )}
        {!collapsed && (
          <button
            id="btn-toggle-sidebar"
            onClick={onToggleCollapse}
            title="Collapse sidebar"
            className="titlebar-no-drag p-1.5 rounded-md text-[#7c8599] hover:text-[#edf0f5] hover:bg-white/[0.06] transition-colors shrink-0"
          >
            <PanelLeftClose className="w-4 h-4" />
          </button>
        )}
      </div>

      {/* Primary Action Strip: Compose + Cmd+K Search */}
      <div className="p-2.5 flex flex-col gap-1.5">
        {collapsed && (
          <button
            id="btn-toggle-sidebar"
            onClick={onToggleCollapse}
            title="Expand sidebar"
            className="h-7 rounded-md text-[#7c8599] hover:text-[#edf0f5] hover:bg-white/[0.06] flex items-center justify-center transition-colors"
          >
            <PanelLeftOpen className="w-4 h-4" />
          </button>
        )}
        <button
          id="btn-compose"
          onClick={onCompose}
          className={`h-8 rounded-md bg-[#edf0f5] hover:bg-white text-[#090a0d] font-medium text-[12.5px] flex items-center justify-center gap-2 transition-all active:scale-[0.99] shadow-sm ${
            collapsed ? 'px-0' : 'px-3 justify-between'
          }`}
          title="Compose new encrypted message (C)"
        >
          <span className="flex items-center gap-2">
            <PenSquare className="w-3.5 h-3.5" strokeWidth={2.2} />
            {!collapsed && <span>Compose</span>}
          </span>
          {!collapsed && (
            <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-black/10 text-black/70">
              C
            </span>
          )}
        </button>

        {!collapsed && (
          <button
            id="btn-open-command-palette"
            onClick={onOpenCommandPalette}
            className="h-8 px-2.5 rounded-md bg-[#12151c] hover:bg-[#171b24] border border-white/[0.07] text-[#7c8599] hover:text-[#c7cfde] text-[12px] flex items-center justify-between transition-colors"
          >
            <span className="flex items-center gap-2">
              <Search className="w-3.5 h-3.5 text-[#596174]" />
              <span>Search &amp; Commands</span>
            </span>
            <span className="kbd">⌘K</span>
          </button>
        )}
      </div>

      {/* Navigation Folders */}
      <nav className="px-2 py-1 flex-1 overflow-y-auto space-y-4">
        <div className="space-y-0.5">
          {!collapsed && (
            <div className="px-2 pb-1 text-[10px] font-medium uppercase tracking-[0.06em] text-[#596174]">
              Mailboxes
            </div>
          )}
          {FOLDERS.map(({ role, label, shortcut, icon: Icon }) => {
            const isActive = activeFolder === role;
            const count = getFolderCount(role);
            const displayLabel =
              role === 'inbox' && activeAccountId !== null ? 'Inbox' : label;

            return (
              <button
                key={role}
                id={`nav-folder-${role}`}
                onClick={() => onSelectFolder(role)}
                title={collapsed ? `${displayLabel} (${shortcut})` : undefined}
                className={`w-full h-8 rounded-md flex items-center transition-colors group ${
                  collapsed ? 'justify-center px-0' : 'justify-between px-2.5'
                } ${
                  isActive
                    ? 'bg-white/[0.075] text-[#f3f6fb] font-medium'
                    : 'text-[#8d96a8] hover:text-[#dce2ec] hover:bg-white/[0.035]'
                }`}
              >
                <span className="flex items-center gap-2.5 min-w-0">
                  <Icon
                    className={`w-3.5 h-3.5 shrink-0 ${
                      isActive ? 'text-emerald-400' : 'text-[#667085] group-hover:text-[#9ba3b5]'
                    }`}
                  />
                  {!collapsed && <span className="text-[12.5px] truncate">{displayLabel}</span>}
                </span>
                {!collapsed && count > 0 && (
                  <span
                    className={`text-[11px] font-mono tabular-nums px-1.5 py-0.5 rounded ${
                      role === 'inbox'
                        ? 'bg-emerald-500/15 text-emerald-300'
                        : 'text-[#667085]'
                    }`}
                  >
                    {count.toLocaleString()}
                  </span>
                )}
              </button>
            );
          })}
        </div>

        {/* Direct Email Accounts */}
        <div className="space-y-1">
          {!collapsed && (
            <div className="px-2 flex items-center justify-between">
              <span className="text-[10px] font-medium uppercase tracking-[0.06em] text-[#596174]">
                Accounts
              </span>
              <div className="flex items-center gap-1">
                <button
                  id="btn-sync-accounts"
                  onClick={onSyncAll}
                  title="Check for new mail"
                  className="p-1 rounded text-[#596174] hover:text-[#edf0f5] hover:bg-white/[0.06] transition-colors"
                >
                  <RefreshCw className={`w-3 h-3 ${syncing ? 'animate-spin text-emerald-400' : ''}`} />
                </button>
                <button
                  id="btn-add-account"
                  onClick={onOpenAccountSetup}
                  title="Connect an email account"
                  className="p-1 rounded text-[#596174] hover:text-[#edf0f5] hover:bg-white/[0.06] transition-colors"
                >
                  <Plus className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
          )}

          {/* Unified All Accounts Filter */}
          {!collapsed && (
            <button
              id="nav-account-all"
              onClick={() => onSelectAccount(null)}
              className={`w-full h-7 px-2.5 rounded-md flex items-center justify-between text-[12px] transition-colors ${
                activeAccountId === null
                  ? 'bg-white/[0.05] text-[#edf0f5]'
                  : 'text-[#7c8599] hover:text-[#d0d7e3] hover:bg-white/[0.03]'
              }`}
            >
              <span className="flex items-center gap-2 truncate">
                <span className="w-1.5 h-1.5 rounded-full bg-[#edf0f5]/80 shrink-0" />
                <span className="truncate">All Accounts</span>
              </span>
            </button>
          )}

          {accounts.map((acc) => {
            const isSelected = activeAccountId === acc.id;
            const idleLive = acc.syncStatus === 'idle';
            return (
              <button
                key={acc.id}
                id={`nav-account-${acc.id}`}
                onClick={() => onSelectAccount(isSelected ? null : acc.id)}
                title={`${acc.email} · Connected`}
                className={`w-full h-9 rounded-md flex items-center transition-colors ${
                  collapsed ? 'justify-center px-0' : 'justify-between px-2.5'
                } ${
                  isSelected
                    ? 'bg-white/[0.075] text-[#f3f6fb]'
                    : 'text-[#8d96a8] hover:text-[#dce2ec] hover:bg-white/[0.035]'
                }`}
              >
                <span className="flex items-center gap-2.5 min-w-0">
                  <span className="relative flex items-center justify-center shrink-0">
                    <span
                      className="w-2 h-2 rounded-full"
                      style={{ backgroundColor: acc.accentColor }}
                    />
                    {idleLive && (
                      <span
                        className="absolute -right-0.5 -bottom-0.5 w-1.5 h-1.5 rounded-full bg-emerald-400 ring-2 ring-[#0d0f14]"
                        title="Live push sync active"
                      />
                    )}
                  </span>
                  {!collapsed && (
                    <span className="flex flex-col items-start min-w-0 leading-tight">
                      <span className="text-[12px] truncate max-w-[140px]">{acc.email}</span>
                      <span className="text-[10.5px] text-[#596174]">
                        {acc.provider === 'proton_bridge'
                          ? 'Proton Mail'
                          : acc.provider === 'gmail'
                            ? 'Gmail'
                            : acc.provider === 'icloud'
                              ? 'iCloud Mail'
                              : 'Custom Email'}
                      </span>
                    </span>
                  )}
                </span>
                {!collapsed && acc.unreadCount > 0 && (
                  <span className="text-[10.5px] font-mono tabular-nums text-[#7c8599]">
                    {acc.unreadCount}
                  </span>
                )}
              </button>
            );
          })}

          {!collapsed && (
            <button
              onClick={onOpenAccountSetup}
              className="w-full h-7 px-2.5 mt-1 rounded-md border border-dashed border-white/[0.09] hover:border-white/[0.2] text-[#7c8599] hover:text-[#edf0f5] text-[11.5px] flex items-center gap-2 transition-colors"
            >
              <Plus className="w-3.5 h-3.5 text-emerald-400" />
              <span>Connect Email Account</span>
            </button>
          )}
        </div>
      </nav>

      {/* Bottom Privacy Telemetry & Vault Status Strip */}
      <div className="p-2.5 border-t border-white/[0.06] space-y-2">
        {!collapsed && (
          <div className="px-2.5 py-2 rounded-md bg-[#11141b] border border-white/[0.06] space-y-1.5">
            <div className="flex items-center justify-between text-[11px]">
              <span className="text-[#8d96a8] flex items-center gap-1.5">
                <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
                Trackers Blocked
              </span>
              <span className="font-mono font-medium text-emerald-300 tabular-nums">
                {privacyStats.totalTrackersBlocked.toLocaleString()}
              </span>
            </div>
            <div className="flex items-center justify-between text-[10.5px] text-[#596174]">
              <span className="flex items-center gap-1.5">
                <KeyRound className="w-3 h-3 text-[#667085]" />
                Encrypted Locally
              </span>
              <span className="text-emerald-400/90 font-medium">
                {privacyStats.sqlcipherActive ? 'Active' : 'Local'}
              </span>
            </div>
          </div>
        )}

        <div className={`flex items-center ${collapsed ? 'flex-col gap-1.5' : 'justify-between'}`}>
          <button
            id="btn-open-settings"
            onClick={onOpenSettings}
            title="Privacy & AI Settings"
            className={`h-7 rounded-md text-[#7c8599] hover:text-[#edf0f5] hover:bg-white/[0.05] text-[12px] flex items-center gap-2 transition-colors ${
              collapsed ? 'w-8 justify-center' : 'px-2 flex-1'
            }`}
          >
            <Settings className="w-3.5 h-3.5" />
            {!collapsed && <span>Settings</span>}
          </button>
          {collapsed && (
            <button
              onClick={onOpenAccountSetup}
              title="Add Account"
              className="w-8 h-7 rounded-md text-[#7c8599] hover:text-[#edf0f5] hover:bg-white/[0.05] flex items-center justify-center"
            >
              <Plus className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </div>
    </aside>
  );
}
