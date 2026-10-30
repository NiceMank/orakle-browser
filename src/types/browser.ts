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
export type AccentColor = 'orakle' | 'cyber' | 'emerald' | 'amber' | 'rose';
export type SearchEngineId = 'duckduckgo' | 'brave' | 'google' | 'qwant' | 'startpage';
export type LayoutMode = 'horizontal' | 'vertical';

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

export interface PersistedData {
  settings: BrowserSettings;
  bookmarks: BookmarkItem[];
  history: HistoryItem[];
  speedDials: SpeedDialItem[];
  scratchpad: string;
  whitelistedHosts: string[];
}

export type SidePanelTab = 'bookmarks' | 'history' | 'downloads' | 'notes' | 'settings' | null;

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
      };
      shield: {
        getStats: () => Promise<ShieldStats>;
        toggleHost: (host: string) => Promise<ShieldStats>;
        onUpdated: (cb: (stats: ShieldStats) => void) => () => void;
      };
      security: {
        auditUrl: (url: string) => Promise<SecurityAuditResult>;
        clearBrowsingData: () => Promise<boolean>;
      };
      downloads: {
        getAll: () => Promise<DownloadItemInfo[]>;
        showInFolder: (savePath: string) => Promise<void>;
        openFile: (savePath: string) => Promise<void>;
        onUpdated: (cb: (list: DownloadItemInfo[]) => void) => () => void;
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
