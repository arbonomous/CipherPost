export type ProviderKind = 'gmail' | 'icloud' | 'proton_bridge' | 'custom';
export type AuthType = 'password' | 'oauth2';
export type TlsMode = 'implicit' | 'starttls';
export type FolderRole = 'inbox' | 'sent' | 'drafts' | 'snoozed' | 'archive' | 'trash';
export type PgpStatus =
  | 'none'
  | 'signed_verified'
  | 'signed_unverified'
  | 'encrypted'
  | 'encrypted_verified';

export interface EmailAddress {
  name: string;
  email: string;
}

export interface Account {
  id: string;
  email: string;
  displayName: string;
  provider: ProviderKind;
  authType: AuthType;
  imapHost: string;
  imapPort: number;
  imapTls: TlsMode;
  smtpHost: string;
  smtpPort: number;
  smtpTls: TlsMode;
  accentColor: string;
  syncStatus: string;
  lastSyncedAt: number | null;
  unreadCount: number;
}

export interface NewAccountInput {
  email: string;
  displayName: string;
  provider: ProviderKind;
  authType: AuthType;
  imapHost: string;
  imapPort: number;
  imapTls: TlsMode;
  smtpHost: string;
  smtpPort: number;
  smtpTls: TlsMode;
  secret: string;
  accentColor?: string;
}

export interface TlsHandshakeStep {
  step: string;
  status: 'passed' | 'failed' | 'skipped';
  detail: string;
  latencyMs: number;
}

export interface ConnectionTestResult {
  success: boolean;
  imapTlsVersion: string | null;
  imapCapabilities: string[];
  smtpVerified: boolean;
  steps: TlsHandshakeStep[];
  error: string | null;
}

export interface BlockedTracker {
  url: string;
  domain: string;
  category:
    | 'spy_pixel'
    | 'tracker_domain'
    | 'css_beacon'
    | 'script_injection'
    | 'remote_stylesheet'
    | string;
  reason: string;
  element: string;
}

export interface AttachmentMeta {
  id: string;
  filename: string;
  mimeType: string;
  sizeBytes: number;
  isInline: boolean;
  sha256: string | null;
}

export interface ThreadSummary {
  id: string;
  accountId: string;
  folderRole: FolderRole | string;
  subject: string;
  snippet: string;
  participants: EmailAddress[];
  lastSenderName: string;
  lastSenderEmail: string;
  messageCount: number;
  unreadCount: number;
  isStarred: boolean;
  hasAttachments: boolean;
  pgpStatus: PgpStatus;
  trackersBlocked: number;
  snoozedUntil: number | null;
  lastMessageAt: number;
}

export interface MessageDetail {
  id: string;
  threadId: string;
  accountId: string;
  folderRole: string;
  messageIdHeader: string | null;
  subject: string;
  senderName: string;
  senderEmail: string;
  to: EmailAddress[];
  cc: EmailAddress[];
  bodyText: string;
  bodyHtmlSanitized: string;
  bodyHtmlWithAssets: string;
  blockedTrackers: BlockedTracker[];
  trackersBlockedCount: number;
  remoteAssetsCount: number;
  pgpStatus: PgpStatus;
  pgpFingerprint: string | null;
  pgpSigner: string | null;
  isRead: boolean;
  isStarred: boolean;
  isBodyFetched: boolean;
  sizeBytes: number;
  receivedAt: number;
  attachments: AttachmentMeta[];
}

export interface ThreadDetail {
  thread: ThreadSummary;
  messages: MessageDetail[];
}

export interface SearchResultItem {
  threadId: string;
  messageId: string;
  accountId: string;
  folderRole: string;
  subject: string;
  senderName: string;
  senderEmail: string;
  highlightedSnippet: string;
  receivedAt: number;
  pgpStatus: PgpStatus;
  trackersBlocked: number;
  rank: number;
}

export interface SearchResponse {
  query: string;
  queryTimeMs: number;
  totalHits: number;
  results: SearchResultItem[];
}

export interface FolderCounts {
  inboxUnread: number;
  inboxTotal: number;
  sentTotal: number;
  draftsTotal: number;
  snoozedTotal: number;
  archiveTotal: number;
  trashTotal: number;
}

export interface PrivacyStats {
  totalTrackersBlocked: number;
  totalMessagesIndexed: number;
  pgpVerifiedCount: number;
  sqlcipherActive: boolean;
  keychainBackend: string;
  fts5Ready: boolean;
}

export interface ComposePayload {
  accountId: string;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  bodyText: string;
  inReplyTo?: string | null;
  threadId?: string | null;
  pgpSign: boolean;
  pgpEncrypt: boolean;
}

export interface AppSettings {
  aiEnabled: boolean;
  aiEndpoint: string;
  aiModel: string;
  blockRemoteImages: boolean;
  stripTrackers: boolean;
  autoVerifyPgp: boolean;
}

export interface AiSummaryResponse {
  threadId: string;
  bullets: string[];
  actionRequired: boolean;
  modelUsed: string;
  latencyMs: number;
}

export interface AiDraftResponse {
  draftText: string;
  modelUsed: string;
  latencyMs: number;
}

export interface AiHealthResponse {
  reachable: boolean;
  endpoint: string;
  availableModels: string[];
  latencyMs: number;
  error: string | null;
}

export interface SyncEventPayload {
  accountId: string;
  phase: 'headers' | 'bodies' | 'idle' | 'error';
  syncedCount: number;
  detail: string;
}
