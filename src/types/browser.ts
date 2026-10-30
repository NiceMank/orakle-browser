export interface Tab {
  id: string;
  title: string;
  url: string;
  favicon?: string;
  isLoading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
  isPinned: boolean;
  isPrivate: boolean;
  isMuted: boolean;
  isAudioPlaying: boolean;
  zoomFactor: number;
  securityState: 'secure' | 'insecure' | 'internal';
  /** Espace de travail auquel l'onglet appartient */
  spaceId: string;
  /** Onglet dupliqué / mis en pause */
  isSuspended?: boolean;
  /** Date de dernière activation (tri de la barre d'onglets) */
  lastActiveAt?: number;
}

export interface BookmarkItem {
  id: string;
  title: string;
  url: string;
  category: string;
  createdAt: number;
}

export interface HistoryItem {
  id: string;
  title: string;
  url: string;
  visitedAt: number;
}

export interface SpeedDialItem {
  id: string;
  title: string;
  url: string;
  color: string;
  badge: string;
}

export type ThemeMode = 'dark' | 'light';
export type AccentColor =
  | 'orakle'
  | 'cyber'
  | 'emerald'
  | 'amber'
  | 'rose'
  | 'ice'
  | 'graphite';
export type SearchEngineId = 'duckduckgo' | 'brave' | 'google' | 'qwant' | 'startpage' | 'perplexity';
export type LayoutMode = 'horizontal' | 'vertical';
export type GlassDensity = 'thin' | 'regular' | 'thick';
export type MotionMode = 'full' | 'calm';
export type AiProvider = 'pollinations' | 'openai-compatible' | 'offline';

export interface BrowserSettings {
  theme: ThemeMode;
  accent: AccentColor;
  searchEngine: SearchEngineId;
  layoutMode: LayoutMode;
  showBookmarksBar: boolean;
  shieldEnabled: boolean;
  stripTrackingParams: boolean;
  sendDoNotTrack: boolean;
  userAgentProfile: string;
  homeUrl: string;

  /* --- Apparence Aurora (v1.1) --- */
  glassDensity: GlassDensity;
  motionMode: MotionMode;
  showAurora: boolean;
  showNoise: boolean;
  showGrid: boolean;
  compactTabsBar: boolean;
  reduceTransparency: boolean;

  /* --- Lecture & confort --- */
  readerFontSize: number;
  readerTheme: 'paper' | 'sepia' | 'night';

  /* --- Assistant Orakle --- */
  aiProvider: AiProvider;
  aiEndpoint: string;
  aiModel: string;
  aiAutoSummarize: boolean;

  /* --- Focus & bien-être --- */
  focusBlockList: string[];
}

export interface BlockedEntry {
  id: string;
  domain: string;
  url: string;
  category: 'Publicité' | 'Traqueur' | 'Télémétrie' | 'Paramètre URL';
  timestamp: number;
}

export interface ShieldStats {
  enabled: boolean;
  stripTrackingParams: boolean;
  sendDoNotTrack: boolean;
  userAgentProfile: string;
  totalBlocked: number;
  trackersBlocked: number;
  adsBlocked: number;
  paramsStripped: number;
  estimatedBytesSaved: number;
  whitelistedHosts: string[];
  recentBlocked: BlockedEntry[];
  topBlockedDomains: Record<string, number>;
}

export interface SecurityAuditResult {
  url: string;
  statusCode: number;
  isHttps: boolean;
  score: number;
  grade: 'A+' | 'A' | 'B' | 'C' | 'D' | 'F';
  headers: Record<string, string>;
  checks: {
    name: string;
    header: string;
    present: boolean;
    value: string;
    description: string;
  }[];
}

/** Mesures de performance réseau (Orakle Pulse) */
export interface NetworkTimingResult {
  url: string;
  ok: boolean;
  statusCode: number;
  totalMs: number;
  dnsMs: number;
  connectMs: number;
  tlsMs: number;
  firstByteMs: number;
  contentLength: number;
  server: string;
  protocol: string;
  cached: boolean;
  error?: string;
}

export interface DownloadItemInfo {
  id: string;
  filename: string;
  url: string;
  savePath: string;
  receivedBytes: number;
  totalBytes: number;
  state: 'progressing' | 'completed' | 'cancelled' | 'interrupted';
  startedAt: number;
}

/** Espace de travail (Workspace / Space) — séparation des contextes */
export interface Space {
  id: string;
  name: string;
  emoji: string;
  color: string;
  createdAt: number;
}

/** Session enregistrée (Snapshot de travail) */
export interface SavedSession {
  id: string;
  name: string;
  emoji: string;
  tabs: { title: string; url: string }[];
  createdAt: number;
}

/** Résultat d'extraction du mode Lecture */
export interface ReaderArticle {
  url: string;
  title: string;
  byline: string;
  siteName: string;
  excerpt: string;
  html: string;
  textContent: string;
  length: number;
  readingMinutes: number;
}

/** Cookie inspecté */
export interface CookieInfo {
  name: string;
  value: string;
  domain: string;
  path: string;
  secure: boolean;
  httpOnly: boolean;
  session: boolean;
  sameSite: string;
  expires: number;
  size: number;
  /** Classification privacy */
  risk: 'ok' | 'tracker' | 'third-party';
}

export interface PersistedData {
  settings: BrowserSettings;
  bookmarks: BookmarkItem[];
  history: HistoryItem[];
  speedDials: SpeedDialItem[];
  scratchpad: string;
  whitelistedHosts: string[];
  spaces: Space[];
  sessions: SavedSession[];
}

export type SidePanelTab =
  | 'bookmarks'
  | 'history'
  | 'downloads'
  | 'notes'
  | 'settings'
  | 'privacy'
  | 'pulse'
  | 'spaces'
  | null;

declare global {
  interface Window {
    orakle?: {
      window: {
        minimize: () => Promise<void>;
        maximize: () => Promise<boolean>;
        close: () => Promise<void>;
        isMaximized: () => Promise<boolean>;
        onMaximizedChange: (cb: (maximized: boolean) => void) => () => void;
      };
      storage: {
        getAll: () => Promise<PersistedData>;
        updateSettings: (patch: Partial<BrowserSettings>) => Promise<BrowserSettings>;
        addBookmark: (bm: { title: string; url: string; category: string }) => Promise<BookmarkItem[]>;
        removeBookmark: (urlOrId: string) => Promise<BookmarkItem[]>;
        addHistory: (entry: { title: string; url: string }) => Promise<HistoryItem[]>;
        clearHistory: () => Promise<HistoryItem[]>;
        removeHistoryItem: (id: string) => Promise<HistoryItem[]>;
        updateSpeedDials: (dials: SpeedDialItem[]) => Promise<SpeedDialItem[]>;
        updateScratchpad: (content: string) => Promise<string>;
        saveSpaces: (spaces: Space[]) => Promise<Space[]>;
        saveSession: (session: { name: string; emoji: string; tabs: { title: string; url: string }[] }) => Promise<SavedSession[]>;
        deleteSession: (id: string) => Promise<SavedSession[]>;
      };
      shield: {
        getStats: () => Promise<ShieldStats>;
        toggleHost: (host: string) => Promise<ShieldStats>;
        onUpdated: (cb: (stats: ShieldStats) => void) => () => void;
      };
      security: {
        auditUrl: (url: string) => Promise<SecurityAuditResult>;
        clearBrowsingData: () => Promise<boolean>;
        clearData: (types: string[]) => Promise<boolean>;
      };
      downloads: {
        getAll: () => Promise<DownloadItemInfo[]>;
        showInFolder: (savePath: string) => Promise<void>;
        openFile: (savePath: string) => Promise<void>;
        onUpdated: (cb: (list: DownloadItemInfo[]) => void) => () => void;
      };
      reader: {
        extract: (url: string) => Promise<ReaderArticle | null>;
        summarize: (text: string) => Promise<{ summary: string; engine: string }>;
      };
      network: {
        probe: (url: string) => Promise<NetworkTimingResult>;
      };
      privacy: {
        cookies: (partition?: string) => Promise<CookieInfo[]>;
        clearCookies: (domain: string, partition?: string) => Promise<boolean>;
      };
      tabs: {
        onOpenFromMain: (
          cb: (payload: { url: string; background?: boolean; isPrivate?: boolean }) => void
        ) => () => void;
      };
    };
  }

  namespace React {
    namespace JSX {
      interface IntrinsicElements {
        webview: React.DetailedHTMLProps<
          React.HTMLAttributes<HTMLElement> & {
            src?: string;
            partition?: string;
            allowpopups?: string | boolean;
            useragent?: string;
          },
          HTMLElement
        >;
      }
    }
  }
}
