import { app } from 'electron';
import fs from 'node:fs';
import path from 'node:path';

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

export interface BrowserSettings {
  theme: 'dark' | 'light';
  accent: 'orakle' | 'cyber' | 'emerald' | 'amber' | 'rose';
  searchEngine: 'duckduckgo' | 'brave' | 'google' | 'qwant' | 'startpage';
  layoutMode: 'horizontal' | 'vertical';
  showBookmarksBar: boolean;
  shieldEnabled: boolean;
  stripTrackingParams: boolean;
  sendDoNotTrack: boolean;
  userAgentProfile: string;
  homeUrl: string;
}

export interface PersistedData {
  settings: BrowserSettings;
  bookmarks: BookmarkItem[];
  history: HistoryItem[];
  speedDials: SpeedDialItem[];
  scratchpad: string;
  whitelistedHosts: string[];
}

const DEFAULT_DATA: PersistedData = {
  settings: {
    theme: 'dark',
    accent: 'orakle',
    searchEngine: 'duckduckgo',
    layoutMode: 'horizontal',
    showBookmarksBar: true,
    shieldEnabled: true,
    stripTrackingParams: true,
    sendDoNotTrack: true,
    userAgentProfile: 'standard',
    homeUrl: 'orakle://newtab',
  },
  bookmarks: [
    {
      id: 'bm-1',
      title: 'GitHub',
      url: 'https://github.com',
      category: 'Développement',
      createdAt: Date.now() - 86400000 * 5,
    },
    {
      id: 'bm-2',
      title: 'MDN Web Docs',
      url: 'https://developer.mozilla.org/fr/',
      category: 'Développement',
      createdAt: Date.now() - 86400000 * 4,
    },
    {
      id: 'bm-3',
      title: 'Hacker News',
      url: 'https://news.ycombinator.com',
      category: 'Tech & Veille',
      createdAt: Date.now() - 86400000 * 3,
    },
    {
      id: 'bm-4',
      title: 'OWASP Top 10',
      url: 'https://owasp.org/www-project-top-ten/',
      category: 'Cybersécurité',
      createdAt: Date.now() - 86400000 * 2,
    },
    {
      id: 'bm-5',
      title: 'YouTube',
      url: 'https://www.youtube.com',
      category: 'Médias',
      createdAt: Date.now() - 86400000,
    },
    {
      id: 'bm-6',
      title: 'Wikipédia FR',
      url: 'https://fr.wikipedia.org',
      category: 'Savoir',
      createdAt: Date.now() - 43200000,
    },
  ],
  history: [],
  speedDials: [
    {
      id: 'sd-1',
      title: 'DuckDuckGo',
      url: 'https://duckduckgo.com',
      color: '#de5833',
      badge: 'DDG',
    },
    {
      id: 'sd-2',
      title: 'GitHub',
      url: 'https://github.com',
      color: '#6e5494',
      badge: 'GH',
    },
    {
      id: 'sd-3',
      title: 'YouTube',
      url: 'https://www.youtube.com',
      color: '#ff0033',
      badge: 'YT',
    },
    {
      id: 'sd-4',
      title: 'MDN Docs',
      url: 'https://developer.mozilla.org/fr/',
      color: '#0a84ff',
      badge: 'MDN',
    },
    {
      id: 'sd-5',
      title: 'Hacker News',
      url: 'https://news.ycombinator.com',
      color: '#ff6600',
      badge: 'HN',
    },
    {
      id: 'sd-6',
      title: 'Wikipédia',
      url: 'https://fr.wikipedia.org',
      color: '#38bdf8',
      badge: 'WIKI',
    },
    {
      id: 'sd-7',
      title: 'OWASP Foundation',
      url: 'https://owasp.org',
      color: '#10b981',
      badge: 'SEC',
    },
    {
      id: 'sd-8',
      title: 'Reddit',
      url: 'https://www.reddit.com',
      color: '#ff4500',
      badge: 'RD',
    },
  ],
  scratchpad: `# Notes Rapides Orakle\n\n- Ctrl+T : Nouvel onglet\n- Ctrl+Shift+N : Onglet privé (Fantôme)\n- Ctrl+K : Palette de commandes\n- Ctrl+B : Afficher/Masquer le panneau latéral\n- Ctrl+\\ : Activer l'écran scindé (Split View)\n`,
  whitelistedHosts: [],
};

export class StorageManager {
  private filePath: string;
  private data: PersistedData;

  constructor() {
    const userDataDir = app.getPath('userData');
    this.filePath = path.join(userDataDir, 'orakle-browser-state.json');
    this.data = this.load();
  }

  private load(): PersistedData {
    try {
      if (fs.existsSync(this.filePath)) {
        const raw = fs.readFileSync(this.filePath, 'utf-8');
        const parsed = JSON.parse(raw) as Partial<PersistedData>;
        return {
          settings: { ...DEFAULT_DATA.settings, ...(parsed.settings || {}) },
          bookmarks: Array.isArray(parsed.bookmarks) ? parsed.bookmarks : DEFAULT_DATA.bookmarks,
          history: Array.isArray(parsed.history) ? parsed.history : [],
          speedDials: Array.isArray(parsed.speedDials) ? parsed.speedDials : DEFAULT_DATA.speedDials,
          scratchpad: typeof parsed.scratchpad === 'string' ? parsed.scratchpad : DEFAULT_DATA.scratchpad,
          whitelistedHosts: Array.isArray(parsed.whitelistedHosts) ? parsed.whitelistedHosts : [],
        };
      }
    } catch (err) {
      console.error('[Orakle Storage] Failed to read state file, falling back to defaults:', err);
    }
    return structuredClone(DEFAULT_DATA);
  }

  private save() {
    try {
      const dir = path.dirname(this.filePath);
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }
      fs.writeFileSync(this.filePath, JSON.stringify(this.data, null, 2), 'utf-8');
    } catch (err) {
      console.error('[Orakle Storage] Failed to persist state:', err);
    }
  }

  public getAll(): PersistedData {
    return this.data;
  }

  public updateSettings(patch: Partial<BrowserSettings>): BrowserSettings {
    this.data.settings = { ...this.data.settings, ...patch };
    this.save();
    return this.data.settings;
  }

  public addBookmark(bookmark: Omit<BookmarkItem, 'id' | 'createdAt'>): BookmarkItem[] {
    const existing = this.data.bookmarks.find((b) => b.url === bookmark.url);
    if (existing) {
      existing.title = bookmark.title || existing.title;
      existing.category = bookmark.category || existing.category;
      this.save();
      return this.data.bookmarks;
    }
    const item: BookmarkItem = {
      id: `bm-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      title: bookmark.title || bookmark.url,
      url: bookmark.url,
      category: bookmark.category || 'Général',
      createdAt: Date.now(),
    };
    this.data.bookmarks = [item, ...this.data.bookmarks];
    this.save();
    return this.data.bookmarks;
  }

  public removeBookmark(urlOrId: string): BookmarkItem[] {
    this.data.bookmarks = this.data.bookmarks.filter((b) => b.id !== urlOrId && b.url !== urlOrId);
    this.save();
    return this.data.bookmarks;
  }

  public addHistory(entry: { title: string; url: string }): HistoryItem[] {
    if (!entry.url || entry.url.startsWith('orakle://') || entry.url === 'about:blank') {
      return this.data.history;
    }
    // Avoid consecutive duplicate entries
    if (this.data.history[0]?.url === entry.url) {
      this.data.history[0].title = entry.title || this.data.history[0].title;
      this.data.history[0].visitedAt = Date.now();
      this.save();
      return this.data.history;
    }
    const item: HistoryItem = {
      id: `hist-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      title: entry.title || entry.url,
      url: entry.url,
      visitedAt: Date.now(),
    };
    this.data.history = [item, ...this.data.history.slice(0, 499)];
    this.save();
    return this.data.history;
  }

  public clearHistory(): HistoryItem[] {
    this.data.history = [];
    this.save();
    return this.data.history;
  }

  public removeHistoryItem(id: string): HistoryItem[] {
    this.data.history = this.data.history.filter((h) => h.id !== id);
    this.save();
    return this.data.history;
  }

  public updateSpeedDials(speedDials: SpeedDialItem[]): SpeedDialItem[] {
    this.data.speedDials = speedDials;
    this.save();
    return this.data.speedDials;
  }

  public updateScratchpad(content: string): string {
    this.data.scratchpad = content;
    this.save();
    return this.data.scratchpad;
  }

  public setWhitelistedHosts(hosts: string[]) {
    this.data.whitelistedHosts = hosts;
    this.save();
  }
}
