import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import {
  X,
  RotateCw,
  Search,
  EyeOff,
  Shield,
  ShieldOff,
  Columns2,
  Folder,
  ChevronRight,
  Moon,
} from 'lucide-react';
import {
  Tab,
  BookmarkItem,
  HistoryItem,
  SpeedDialItem,
  BrowserSettings,
  ShieldStats,
  DownloadItemInfo,
  SidePanelTab,
  Space,
  SavedSession,
} from './types/browser';
import {
  resolveOmniboxInput,
  getSecurityState,
  extractHostname,
  prettyUrl,
} from './utils/url';
import { watchBrowserVitals, initialVitals, BrowserVitals } from './utils/vitals';
import { NewTabPage } from './components/NewTabPage';
import { ShieldModal } from './components/ShieldModal';
import { SidePanel } from './components/SidePanel';
import { CommandPalette } from './components/CommandPalette';
import { AuroraBackground } from './components/AuroraBackground';
import { TopChrome } from './components/TopChrome';
import { SpaceRail } from './components/SpaceRail';
import { ReaderView } from './components/ReaderView';
import { FocusMode } from './components/FocusMode';

interface WebviewElement extends HTMLElement {
  src: string;
  loadURL: (url: string) => Promise<void>;
  reload: () => void;
  stop: () => void;
  goBack: () => void;
  goForward: () => void;
  canGoBack: () => boolean;
  canGoForward: () => boolean;
  getURL: () => string;
  getTitle: () => string;
  setAudioMuted: (muted: boolean) => void;
  isAudioMuted: () => boolean;
  setZoomFactor: (factor: number) => void;
  openDevTools: () => void;
  findInPage: (text: string, options?: { forward?: boolean; findNext?: boolean }) => number;
  stopFindInPage: (action: 'clearSelection' | 'keepSelection' | 'activateSelection') => void;
}

const NEWTAB = 'orakle://newtab';

const createTabObject = (url = NEWTAB, isPrivate = false, spaceId = 'space-perso'): Tab => ({
  id: `tab-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
  title: url === NEWTAB ? 'Nouvel onglet' : extractHostname(url),
  url,
  isLoading: false,
  canGoBack: false,
  canGoForward: false,
  isPinned: false,
  isPrivate,
  isMuted: false,
  isAudioPlaying: false,
  zoomFactor: 1,
  securityState: getSecurityState(url),
  spaceId,
  lastActiveAt: Date.now(),
});

const DEFAULT_SPEED_DIALS: SpeedDialItem[] = [
  { id: 'sd-1', title: 'DuckDuckGo', url: 'https://duckduckgo.com', color: '#de5833', badge: 'DDG' },
  { id: 'sd-2', title: 'GitHub', url: 'https://github.com', color: '#6e5494', badge: 'GH' },
  { id: 'sd-3', title: 'YouTube', url: 'https://www.youtube.com', color: '#ff0033', badge: 'YT' },
  { id: 'sd-4', title: 'MDN Docs', url: 'https://developer.mozilla.org/fr/', color: '#0a84ff', badge: 'MDN' },
  { id: 'sd-5', title: 'Hacker News', url: 'https://news.ycombinator.com', color: '#ff6600', badge: 'HN' },
  { id: 'sd-6', title: 'Wikipédia', url: 'https://fr.wikipedia.org', color: '#38bdf8', badge: 'WIKI' },
  { id: 'sd-7', title: 'OWASP', url: 'https://owasp.org', color: '#10b981', badge: 'SEC' },
  { id: 'sd-8', title: 'Perplexity', url: 'https://www.perplexity.ai', color: '#20b8cd', badge: 'AI' },
];

const DEFAULT_BOOKMARKS: BookmarkItem[] = [
  { id: 'bm-1', title: 'GitHub', url: 'https://github.com', category: 'Développement', createdAt: Date.now() },
  { id: 'bm-2', title: 'MDN Web Docs', url: 'https://developer.mozilla.org/fr/', category: 'Développement', createdAt: Date.now() },
  { id: 'bm-3', title: 'Hacker News', url: 'https://news.ycombinator.com', category: 'Veille', createdAt: Date.now() },
  { id: 'bm-4', title: 'OWASP Top 10', url: 'https://owasp.org/www-project-top-ten/', category: 'Cybersécurité', createdAt: Date.now() },
];

const DEFAULT_SETTINGS: BrowserSettings = {
  theme: 'dark',
  accent: 'orakle',
  searchEngine: 'duckduckgo',
  layoutMode: 'horizontal',
  showBookmarksBar: true,
  shieldEnabled: true,
  stripTrackingParams: true,
  sendDoNotTrack: true,
  userAgentProfile: 'standard',
  homeUrl: NEWTAB,
  glassDensity: 'regular',
  motionMode: 'full',
  showAurora: true,
  showNoise: true,
  showGrid: false,
  compactTabsBar: false,
  reduceTransparency: false,
  readerFontSize: 17,
  readerTheme: 'paper',
  aiProvider: 'pollinations',
  aiEndpoint: 'https://api.openai.com/v1/chat/completions',
  aiModel: 'openai',
  aiAutoSummarize: false,
  focusBlockList: ['x.com', 'twitter.com', 'instagram.com', 'tiktok.com'],
};

export const App: React.FC = () => {
  /* ---------------- Onglets & navigation ---------------- */
  const [tabs, setTabs] = useState<Tab[]>(() => [createTabObject(NEWTAB, false, 'space-perso')]);
  const [activeTabId, setActiveTabId] = useState<string>(() => tabs[0].id);
  const [splitTabId, setSplitTabId] = useState<string | null>(null);
  const [closedTabsStack, setClosedTabsStack] = useState<{ url: string; isPrivate: boolean; spaceId: string }[]>([]);

  /* ---------------- Omnibox ---------------- */
  const [omniboxText, setOmniboxText] = useState('');
  const [omniboxFocused, setOmniboxFocused] = useState(false);

  /* ---------------- Recherche & modes ---------------- */
  const [showFindBar, setShowFindBar] = useState(false);
  const [findText, setFindText] = useState('');
  const [findResults, setFindResults] = useState<number | null>(null);
  const [zenMode, setZenMode] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);

  /* ---------------- Panneaux & modales ---------------- */
  const [sidePanel, setSidePanel] = useState<SidePanelTab>(null);
  const [shieldModalOpen, setShieldModalOpen] = useState(false);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [readerOpen, setReaderOpen] = useState(false);
  const [focusOpen, setFocusOpen] = useState(false);
  const [toast, setToast] = useState<{ text: string; tone: 'info' | 'ok' | 'warn' } | null>(null);

  /* ---------------- Données persistantes ---------------- */
  const [settings, setSettings] = useState<BrowserSettings>(DEFAULT_SETTINGS);
  const [bookmarks, setBookmarks] = useState<BookmarkItem[]>(DEFAULT_BOOKMARKS);
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [speedDials, setSpeedDials] = useState<SpeedDialItem[]>(DEFAULT_SPEED_DIALS);
  const [scratchpad, setScratchpad] = useState('');
  const [downloads, setDownloads] = useState<DownloadItemInfo[]>([]);
  const [spaces, setSpaces] = useState<Space[]>([
    { id: 'space-perso', name: 'Personnel', emoji: '🌙', color: '#0a84ff', createdAt: Date.now() },
    { id: 'space-work', name: 'Travail', emoji: '💼', color: '#8b5cf6', createdAt: Date.now() },
    { id: 'space-veille', name: 'Veille', emoji: '🛰️', color: '#10b981', createdAt: Date.now() },
  ]);
  const [activeSpaceId, setActiveSpaceId] = useState('space-perso');
  const [sessions, setSessions] = useState<SavedSession[]>([]);
  const [vitals, setVitals] = useState<BrowserVitals>(initialVitals);
  const [shieldStats, setShieldStats] = useState<ShieldStats>({
    enabled: true,
    stripTrackingParams: true,
    sendDoNotTrack: true,
    userAgentProfile: 'standard',
    totalBlocked: 0,
    trackersBlocked: 0,
    adsBlocked: 0,
    paramsStripped: 0,
    estimatedBytesSaved: 0,
    whitelistedHosts: [],
    recentBlocked: [],
    topBlockedDomains: {},
  });

  /* ---------------- Refs ---------------- */
  const webviewRefs = useRef<Record<string, WebviewElement | null>>({});
  const initialWebviewUrlRef = useRef<Record<string, string>>({});
  const attachedListenersRef = useRef<Set<string>>(new Set());
  const omniboxInputRef = useRef<HTMLInputElement | null>(null);
  const lastNavUrlRef = useRef<Record<string, string>>({});

  const visibleTabs = useMemo(() => tabs.filter((t) => t.spaceId === activeSpaceId), [tabs, activeSpaceId]);
  const activeTab = tabs.find((t) => t.id === activeTabId) || visibleTabs[0] || tabs[0];
  const isCurrentBookmarked = bookmarks.some((b) => b.url === activeTab?.url);

  const notify = useCallback((text: string, tone: 'info' | 'ok' | 'warn' = 'info') => {
    setToast({ text, tone });
    setTimeout(() => setToast(null), 2600);
  }, []);

  /* ---------------- Thème & réglages visuels sur le DOM ---------------- */
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', settings.theme);
    document.documentElement.setAttribute('data-motion', settings.motionMode);
    document.body.setAttribute('data-accent', settings.accent);
    const blur =
      settings.glassDensity === 'thin' ? '14px' : settings.glassDensity === 'thick' ? '34px' : '24px';
    document.documentElement.style.setProperty('--glass-blur', blur);
    document.documentElement.setAttribute(
      'data-glass',
      settings.reduceTransparency ? 'solid' : 'liquid'
    );
  }, [
    settings.theme,
    settings.accent,
    settings.glassDensity,
    settings.motionMode,
    settings.reduceTransparency,
  ]);

  /* ---------------- Chargement initial ---------------- */
  useEffect(() => {
    if (!window.orakle) return;

    window.orakle.storage.getAll().then((data) => {
      setSettings({ ...DEFAULT_SETTINGS, ...data.settings });
      setBookmarks(data.bookmarks);
      setHistory(data.history);
      setSpeedDials(data.speedDials);
      setScratchpad(data.scratchpad);
      if (data.spaces?.length) {
        setSpaces(data.spaces);
        setActiveSpaceId((prev) => (data.spaces.some((s) => s.id === prev) ? prev : data.spaces[0].id));
      }
      if (data.sessions) setSessions(data.sessions);
    });

    window.orakle.shield.getStats().then(setShieldStats);
    window.orakle.downloads.getAll().then(setDownloads);
    window.orakle.window.isMaximized().then(setIsMaximized);

    const unsubShield = window.orakle.shield.onUpdated(setShieldStats);
    const unsubDownloads = window.orakle.downloads.onUpdated(setDownloads);
    const unsubMax = window.orakle.window.onMaximizedChange(setIsMaximized);
    const stopVitals = watchBrowserVitals(setVitals);

    return () => {
      unsubShield();
      unsubDownloads();
      unsubMax();
      stopVitals();
    };
  }, []);

  /* ---------------- Synchronisation de l'omnibox ---------------- */
  useEffect(() => {
    if (!activeTab) return;
    setOmniboxText(activeTab.url === NEWTAB ? '' : activeTab.url);
  }, [activeTab?.id, activeTab?.url]);

  /* ---------------- Navigation ---------------- */
  const navigateTab = useCallback(
    (tabId: string, rawInput: string) => {
      const targetUrl = resolveOmniboxInput(rawInput, settings.searchEngine);
      const host = extractHostname(targetUrl);

      // Orakle Focus : blocage des domaines distractants
      const blocked = settings.focusBlockList.some(
        (d) => host === d.replace(/^www\./, '') || host.endsWith(`.${d.replace(/^www\./, '')}`)
      );
      if (blocked) {
        notify(`« ${host} » est bloqué par le Mode Focus`, 'warn');
        setFocusOpen(true);
        return;
      }

      setTabs((prev) =>
        prev.map((t) =>
          t.id === tabId
            ? {
                ...t,
                url: targetUrl,
                title: targetUrl === NEWTAB ? 'Nouvel onglet' : extractHostname(targetUrl),
                securityState: getSecurityState(targetUrl),
                isSuspended: false,
                lastActiveAt: Date.now(),
              }
            : t
        )
      );
      lastNavUrlRef.current[tabId] = targetUrl;

      if (targetUrl !== NEWTAB) {
        const wv = webviewRefs.current[tabId];
        if (wv && typeof wv.loadURL === 'function') {
          wv.loadURL(targetUrl).catch(() => {});
        } else {
          initialWebviewUrlRef.current[tabId] = targetUrl;
        }
      }
    },
    [settings.searchEngine, settings.focusBlockList, notify]
  );

  const handleNewTab = useCallback(
    (url = NEWTAB, isPrivate = false, background = false, spaceId?: string) => {
      const newTab = createTabObject(url, isPrivate, spaceId || activeSpaceId);
      if (url !== NEWTAB) initialWebviewUrlRef.current[newTab.id] = url;
      setTabs((prev) => [...prev, newTab]);
      if (!background) {
        setActiveTabId(newTab.id);
        setActiveSpaceId(newTab.spaceId);
      }
      return newTab.id;
    },
    [activeSpaceId]
  );

  useEffect(() => {
    if (!window.orakle) return;
    return window.orakle.tabs.onOpenFromMain(({ url, background, isPrivate }) => {
      handleNewTab(url, Boolean(isPrivate), Boolean(background));
    });
  }, [handleNewTab]);

  /* ---------------- Fermeture / sommeil ---------------- */
  const handleCloseTab = useCallback(
    (tabId: string, e?: React.MouseEvent) => {
      e?.stopPropagation();
      const target = tabs.find((t) => t.id === tabId);
      if (target && target.url !== NEWTAB) {
        setClosedTabsStack((prev) => [
          { url: target.url, isPrivate: target.isPrivate, spaceId: target.spaceId },
          ...prev.slice(0, 24),
        ]);
      }

      delete webviewRefs.current[tabId];
      delete initialWebviewUrlRef.current[tabId];
      attachedListenersRef.current.delete(tabId);
      if (splitTabId === tabId) setSplitTabId(null);

      const remainingInSpace = tabs.filter((t) => t.id !== tabId && t.spaceId === (target?.spaceId || activeSpaceId));

      if (remainingInSpace.length === 0) {
        const fresh = createTabObject(NEWTAB, false, target?.spaceId || activeSpaceId);
        setTabs((prev) => [...prev.filter((t) => t.id !== tabId), fresh]);
        setActiveTabId(fresh.id);
        return;
      }

      const nextTabs = tabs.filter((t) => t.id !== tabId);
      setTabs(nextTabs);
      if (activeTabId === tabId) {
        setActiveTabId(remainingInSpace[remainingInSpace.length - 1].id);
      }
    },
    [tabs, activeTabId, splitTabId, activeSpaceId]
  );

  const handleSleepTab = useCallback(
    (tabId: string) => {
      setTabs((prev) =>
        prev.map((t) =>
          t.id === tabId
            ? { ...t, isSuspended: true, url: t.url, title: t.title, isLoading: false }
            : t
        )
      );
      notify('Onglet mis en veille — mémoire libérée', 'ok');
    },
    [notify]
  );

  /* ---------------- Attachement des webviews ---------------- */
  const registerWebview = useCallback(
    (tabId: string, isPrivate: boolean, el: WebviewElement | null) => {
      webviewRefs.current[tabId] = el;
      if (!el || attachedListenersRef.current.has(tabId)) return;
      attachedListenersRef.current.add(tabId);

      const updateNavState = () => {
        try {
          const currentUrl = el.getURL();
          const currentTitle = el.getTitle();
          const canGoBack = el.canGoBack();
          const canGoForward = el.canGoForward();

          setTabs((prev) =>
            prev.map((t) =>
              t.id === tabId
                ? {
                    ...t,
                    url: currentUrl || t.url,
                    title: currentTitle || t.title,
                    canGoBack,
                    canGoForward,
                    securityState: getSecurityState(currentUrl || t.url),
                  }
                : t
            )
          );

          if (!isPrivate && currentUrl && !currentUrl.startsWith('orakle://') && currentUrl !== 'about:blank') {
            window.orakle?.storage
              .addHistory({
                title: currentTitle || extractHostname(currentUrl),
                url: currentUrl,
              })
              .then(setHistory);
          }
        } catch {
          /* webview pas encore prête */
        }
      };

      el.addEventListener('did-start-loading', () => {
        setTabs((prev) => prev.map((t) => (t.id === tabId ? { ...t, isLoading: true } : t)));
      });

      el.addEventListener('did-stop-loading', () => {
        setTabs((prev) => prev.map((t) => (t.id === tabId ? { ...t, isLoading: false } : t)));
        updateNavState();
      });

      el.addEventListener('did-navigate', updateNavState);
      el.addEventListener('did-navigate-in-page', updateNavState);
      el.addEventListener('did-fail-load', () =>
        setTabs((prev) => prev.map((t) => (t.id === tabId ? { ...t, isLoading: false } : t)))
      );

      el.addEventListener('page-title-updated', (ev: Event) => {
        const customEv = ev as CustomEvent & { title?: string };
        if (customEv.title) {
          setTabs((prev) => prev.map((t) => (t.id === tabId ? { ...t, title: customEv.title! } : t)));
        }
      });

      el.addEventListener('page-favicon-updated', (ev: Event) => {
        const customEv = ev as CustomEvent & { favicons?: string[] };
        if (customEv.favicons?.length) {
          setTabs((prev) =>
            prev.map((t) => (t.id === tabId ? { ...t, favicon: customEv.favicons![0] } : t))
          );
        }
      });

      el.addEventListener('media-started-playing', () =>
        setTabs((prev) => prev.map((t) => (t.id === tabId ? { ...t, isAudioPlaying: true } : t)))
      );
      el.addEventListener('media-paused', () =>
        setTabs((prev) => prev.map((t) => (t.id === tabId ? { ...t, isAudioPlaying: false } : t)))
      );

      el.addEventListener('found-in-page', (ev: Event) => {
        const customEv = ev as CustomEvent & { result?: { matches?: number } };
        if (typeof customEv.result?.matches === 'number') setFindResults(customEv.result.matches);
      });
    },
    []
  );

  /* ---------------- Actions de navigation ---------------- */
  const handleGoBack = () => {
    const wv = webviewRefs.current[activeTabId];
    if (wv?.canGoBack()) wv.goBack();
  };
  const handleGoForward = () => {
    const wv = webviewRefs.current[activeTabId];
    if (wv?.canGoForward()) wv.goForward();
  };
  const handleReload = () => {
    const wv = webviewRefs.current[activeTabId];
    if (!wv) return;
    if (activeTab.isLoading) wv.stop();
    else wv.reload();
  };
  const handleHome = () => navigateTab(activeTabId, settings.homeUrl || NEWTAB);

  const handleToggleMute = (tabId: string, e?: React.MouseEvent) => {
    e?.stopPropagation();
    const wv = webviewRefs.current[tabId];
    setTabs((prev) =>
      prev.map((t) => {
        if (t.id !== tabId) return t;
        const nextMuted = !t.isMuted;
        if (wv && typeof wv.setAudioMuted === 'function') wv.setAudioMuted(nextMuted);
        return { ...t, isMuted: nextMuted };
      })
    );
  };

  const handleTogglePin = (tabId: string, e?: React.MouseEvent) => {
    e?.stopPropagation();
    setTabs((prev) => prev.map((t) => (t.id === tabId ? { ...t, isPinned: !t.isPinned } : t)));
  };

  const handleZoom = (delta: number) => {
    const wv = webviewRefs.current[activeTabId];
    setTabs((prev) =>
      prev.map((t) => {
        if (t.id !== activeTabId) return t;
        const nextZoom =
          delta === 0 ? 1 : Math.min(2.5, Math.max(0.5, +(t.zoomFactor + delta).toFixed(2)));
        if (wv && typeof wv.setZoomFactor === 'function') wv.setZoomFactor(nextZoom);
        return { ...t, zoomFactor: nextZoom };
      })
    );
  };

  const handleToggleSplitView = useCallback(() => {
    if (splitTabId) {
      setSplitTabId(null);
      return;
    }
    const other = visibleTabs.find((t) => t.id !== activeTabId);
    if (other) {
      setSplitTabId(other.id);
    } else {
      const id = handleNewTab('https://fr.wikipedia.org');
      setSplitTabId(id);
    }
  }, [splitTabId, visibleTabs, activeTabId, handleNewTab]);

  const handleSplitWith = useCallback(
    (tabId: string) => {
      if (tabId === activeTabId) return;
      setSplitTabId(tabId);
    },
    [activeTabId]
  );

  const handleOpenDevTools = useCallback(() => {
    const wv = webviewRefs.current[activeTabId];
    if (wv && typeof wv.openDevTools === 'function') wv.openDevTools();
  }, [activeTabId]);

  /* ---------------- Paramètres, favoris, espaces ---------------- */
  const handleUpdateSettings = useCallback((patch: Partial<BrowserSettings>) => {
    setSettings((prev) => ({ ...prev, ...patch }));
    window.orakle?.storage.updateSettings(patch).then((s) => setSettings({ ...DEFAULT_SETTINGS, ...s }));
  }, []);

  const handleToggleBookmarkCurrent = () => {
    if (!activeTab || activeTab.url.startsWith('orakle://')) return;
    if (isCurrentBookmarked) {
      window.orakle?.storage.removeBookmark(activeTab.url).then(setBookmarks);
      notify('Favori retiré', 'info');
    } else {
      window.orakle?.storage
        .addBookmark({
          title: activeTab.title || extractHostname(activeTab.url),
          url: activeTab.url,
          category: 'Favoris',
        })
        .then((list) => {
          setBookmarks(list);
          notify('Page ajoutée aux favoris', 'ok');
        });
    }
  };

  const handleSaveSpaces = useCallback((next: Space[]) => {
    setSpaces(next);
    window.orakle?.storage.saveSpaces(next);
  }, []);

  const handleSwitchSpace = useCallback(
    (spaceId: string) => {
      setActiveSpaceId(spaceId);
      const target = tabs.find((t) => t.spaceId === spaceId);
      if (target) setActiveTabId(target.id);
      else {
        const fresh = createTabObject(NEWTAB, false, spaceId);
        setTabs((prev) => [...prev, fresh]);
        setActiveTabId(fresh.id);
      }
      setSplitTabId(null);
    },
    [tabs]
  );

  const snapshotSession = useCallback(
    (name?: string, emoji = '🗂️') => {
      const payload = {
        name: name || `Session ${new Date().toLocaleString('fr-FR', { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' })}`,
        emoji,
        tabs: visibleTabs
          .filter((t) => t.url !== NEWTAB)
          .map((t) => ({ title: t.title, url: t.url })),
      };
      if (payload.tabs.length === 0) {
        notify('Aucun onglet à capturer dans cet espace', 'warn');
        return;
      }
      window.orakle?.storage.saveSession(payload).then((list) => {
        setSessions(list);
        notify(`Session « ${payload.name} » enregistrée`, 'ok');
      });
    },
    [visibleTabs, notify]
  );

  const handleSaveSessionFromPanel = (session: {
    name: string;
    emoji: string;
    tabs: { title: string; url: string }[];
  }) => {
    window.orakle?.storage.saveSession(session).then((list) => {
      setSessions(list);
      notify(`Session « ${session.name} » enregistrée`, 'ok');
    });
  };

  const handleDeleteSession = (id: string) => {
    window.orakle?.storage.deleteSession(id).then(setSessions);
  };

  const handleRestoreSession = (session: SavedSession) => {
    session.tabs.forEach((t, i) => handleNewTab(t.url, false, i > 0));
    notify(`${session.tabs.length} onglets restaurés`, 'ok');
  };

  const handleOpenUrls = (urls: string[]) => {
    urls.forEach((u, i) => handleNewTab(u, false, i > 0));
    notify(`${urls.length} onglets ouverts`, 'ok');
  };

  /* ---------------- Raccourcis clavier ---------------- */
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();

      if (mod && key === 't' && e.shiftKey) {
        e.preventDefault();
        if (closedTabsStack.length > 0) {
          const [last, ...rest] = closedTabsStack;
          setClosedTabsStack(rest);
          const id = handleNewTab(last.url, last.isPrivate, false, last.spaceId);
          setActiveTabId(id);
        }
      } else if (mod && key === 't') {
        e.preventDefault();
        handleNewTab(NEWTAB);
      } else if (mod && e.shiftKey && key === 'n') {
        e.preventDefault();
        handleNewTab(NEWTAB, true);
      } else if (mod && key === 'w') {
        e.preventDefault();
        handleCloseTab(activeTabId);
      } else if (mod && key === 'k') {
        e.preventDefault();
        setCommandPaletteOpen((v) => !v);
      } else if (mod && key === 'l') {
        e.preventDefault();
        omniboxInputRef.current?.focus();
        omniboxInputRef.current?.select();
      } else if (mod && key === 'f') {
        e.preventDefault();
        setShowFindBar((v) => !v);
      } else if (mod && key === '\\') {
        e.preventDefault();
        handleToggleSplitView();
      } else if (mod && e.shiftKey && key === 'r') {
        e.preventDefault();
        setReaderOpen((v) => !v);
      } else if (mod && key === 'r') {
        e.preventDefault();
        handleReload();
      } else if (mod && key === 'b') {
        e.preventDefault();
        setSidePanel((p) => (p === 'bookmarks' ? null : 'bookmarks'));
      } else if (mod && key === 'h') {
        e.preventDefault();
        setSidePanel((p) => (p === 'history' ? null : 'history'));
      } else if (mod && key === 'p') {
        e.preventDefault();
        setSidePanel((p) => (p === 'pulse' ? null : 'pulse'));
      } else if (mod && key >= '1' && key <= '9') {
        e.preventDefault();
        const idx = Number(key) - 1;
        if (visibleTabs[idx]) setActiveTabId(visibleTabs[idx].id);
      } else if (mod && key === 'tab') {
        e.preventDefault();
        const idx = visibleTabs.findIndex((t) => t.id === activeTabId);
        const next = visibleTabs[(idx + (e.shiftKey ? -1 : 1) + visibleTabs.length) % visibleTabs.length];
        if (next) setActiveTabId(next.id);
      } else if (e.key === 'F12') {
        e.preventDefault();
        handleOpenDevTools();
      } else if (e.key === 'F11') {
        e.preventDefault();
        setZenMode((z) => !z);
      } else if (e.key === 'Escape') {
        setShowFindBar(false);
        setCommandPaletteOpen(false);
        setReaderOpen(false);
        setFocusOpen(false);
        webviewRefs.current[activeTabId]?.stopFindInPage('clearSelection');
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [
    activeTabId,
    closedTabsStack,
    visibleTabs,
    handleNewTab,
    handleCloseTab,
    handleToggleSplitView,
    handleOpenDevTools,
  ]);

  /* ---------------- Recherche dans la page ---------------- */
  const runFindInPage = (forward = true) => {
    if (!findText.trim()) return;
    const wv = webviewRefs.current[activeTabId];
    if (wv && typeof wv.findInPage === 'function') {
      wv.findInPage(findText, { forward, findNext: true });
    }
  };

  const tabCounts = useMemo(
    () =>
      tabs.reduce<Record<string, number>>((acc, t) => {
        acc[t.spaceId] = (acc[t.spaceId] || 0) + 1;
        return acc;
      }, {}),
    [tabs]
  );

  const canOpenReader = Boolean(activeTab && !activeTab.url.startsWith('orakle://'));

  /* ============================================================
     RENDU
     ============================================================ */
  return (
    <div className="relative flex w-screen h-screen overflow-hidden bg-[var(--bg-base)] text-[var(--text)]">
      <AuroraBackground
        showAurora={settings.showAurora}
        showNoise={settings.showNoise}
        showGrid={settings.showGrid}
        isPrivate={activeTab?.isPrivate}
      />

      <div className="relative z-10 flex w-full h-full">
        {/* Rail des espaces */}
        {!zenMode && (
          <SpaceRail
            spaces={spaces}
            activeSpaceId={activeSpaceId}
            tabCounts={tabCounts}
            onSelect={handleSwitchSpace}
            onSaveSpaces={handleSaveSpaces}
          />
        )}

        <div className="flex flex-col flex-1 min-w-0">
          {/* Chrome supérieur */}
          {!zenMode && (
            <TopChrome
              tabs={visibleTabs}
              activeTab={activeTab}
              activeTabId={activeTabId}
              splitTabId={splitTabId}
              settings={settings}
              omniboxText={omniboxText}
              omniboxFocused={omniboxFocused}
              omniboxInputRef={omniboxInputRef}
              onOmniboxChange={setOmniboxText}
              onOmniboxSubmit={(v) => {
                navigateTab(activeTabId, v);
                setOmniboxText(v);
              }}
              onOmniboxFocusChange={setOmniboxFocused}
              onSelectTab={(id) => {
                setActiveTabId(id);
                setTabs((prev) => prev.map((t) => (t.id === id ? { ...t, lastActiveAt: Date.now() } : t)));
              }}
              onCloseTab={handleCloseTab}
              onNewTab={(isPrivate) => handleNewTab(NEWTAB, Boolean(isPrivate))}
              onToggleMute={handleToggleMute}
              onTogglePin={handleTogglePin}
              onSleepTab={handleSleepTab}
              onSplitWith={handleSplitWith}
              onBack={handleGoBack}
              onForward={handleGoForward}
              onReload={handleReload}
              onHome={handleHome}
              onStop={() => webviewRefs.current[activeTabId]?.stop()}
              onToggleSplit={handleToggleSplitView}
              onShowFind={() => setShowFindBar((v) => !v)}
              showFind={showFindBar}
              onZoom={handleZoom}
              onOpenPalette={() => setCommandPaletteOpen(true)}
              onTogglePanel={(p) => setSidePanel((cur) => (cur === p ? null : (p as SidePanelTab)))}
              activePanel={sidePanel}
              onOpenShield={() => setShieldModalOpen(true)}
              onOpenReader={() => setReaderOpen(true)}
              onOpenFocus={() => setFocusOpen(true)}
              onToggleBookmark={handleToggleBookmarkCurrent}
              isBookmarked={isCurrentBookmarked}
              canOpenReader={canOpenReader}
              shieldEnabled={shieldStats.enabled}
              shieldBlocked={shieldStats.totalBlocked}
              downloadsActive={downloads.some((d) => d.state === 'progressing')}
              isMaximized={isMaximized}
              onMinimize={() => window.orakle?.window.minimize()}
              onMaximize={() => window.orakle?.window.maximize()}
              onCloseWindow={() => window.orakle?.window.close()}
              onUpdateSettings={handleUpdateSettings}
              onToggleTheme={() =>
                handleUpdateSettings({ theme: settings.theme === 'dark' ? 'light' : 'dark' })
              }
              bookmarks={bookmarks}
              history={history}
            />
          )}

          {/* Barre latérale verticale (mode Arc) */}
          {!zenMode && settings.layoutMode === 'vertical' && (
            <div className="flex flex-1 min-h-0">
              <aside className="glass-panel w-60 h-full border-r border-[var(--stroke)] p-2 flex flex-col justify-between shrink-0 z-20">
                <div className="space-y-1 overflow-y-auto flex-1 pr-1">
                  <div className="px-2 py-1.5 text-[9.5px] font-bold uppercase tracking-[0.16em] text-[var(--text-3)] flex items-center justify-between">
                    <span>
                      {spaces.find((s) => s.id === activeSpaceId)?.emoji}{' '}
                      {spaces.find((s) => s.id === activeSpaceId)?.name}
                    </span>
                    <span className="text-[9px]">{visibleTabs.length}</span>
                  </div>
                  {visibleTabs.map((tab) => (
                    <button
                      key={tab.id}
                      type="button"
                      onClick={() => setActiveTabId(tab.id)}
                      onAuxClick={(e) => e.button === 1 && handleCloseTab(tab.id)}
                      className="rail-tab text-left"
                      data-active={tab.id === activeTabId}
                      data-private={tab.isPrivate}
                      title={tab.url}
                    >
                      {tab.isLoading ? (
                        <RotateCw className="w-3.5 h-3.5 animate-spin text-[var(--accent)] shrink-0" />
                      ) : tab.isPrivate ? (
                        <EyeOff className="w-3.5 h-3.5 text-[var(--c-cyber)] shrink-0" />
                      ) : tab.favicon ? (
                        <img src={tab.favicon} alt="" className="w-3.5 h-3.5 rounded shrink-0" />
                      ) : (
                        <Folder className="w-3.5 h-3.5 text-[var(--accent)] shrink-0" />
                      )}
                      <span className="truncate flex-1 text-[11.5px] font-medium">
                        {tab.title || extractHostname(tab.url)}
                      </span>
                      {(tab.isAudioPlaying || tab.isMuted) && (
                        <span className="w-1.5 h-1.5 rounded-full bg-[var(--accent)] shrink-0" />
                      )}
                      <span
                        onClick={(e) => handleCloseTab(tab.id, e)}
                        className="opacity-0 hover:opacity-100 text-[var(--text-3)] hover:text-[var(--text)] shrink-0 cursor-pointer"
                      >
                        <X className="w-3 h-3" />
                      </span>
                    </button>
                  ))}
                </div>
                <div className="pt-2 border-t border-[var(--stroke)] space-y-1.5">
                  <button
                    type="button"
                    onClick={() => handleNewTab(NEWTAB)}
                    className="btn-primary w-full !py-2"
                  >
                    <ChevronRight className="w-3.5 h-3.5" />
                    Nouvel onglet
                  </button>
                  <button
                    type="button"
                    onClick={() => setSidePanel('spaces')}
                    className="btn-glass w-full !justify-center !py-1.5"
                  >
                    <Columns2 className="w-3 h-3" />
                    Sessions
                  </button>
                </div>
              </aside>

              <main className="flex-1 h-full flex gap-1.5 p-1.5 overflow-hidden">
                {renderViewports()}
              </main>
            </div>
          )}

          {/* Zone principale (mode horizontal) */}
          {settings.layoutMode === 'horizontal' && (
            <div className="flex flex-1 min-h-0 overflow-hidden">
              <main className="flex-1 h-full flex gap-1.5 p-1.5 overflow-hidden">{renderViewports()}</main>
            </div>
          )}

          {/* Barre latérale de recherche */}
          {showFindBar && (
            <div className="fixed top-[130px] right-8 z-40 glass-panel-strong px-3 py-2 rounded-2xl flex items-center gap-2 animate-pop">
              <Search className="w-3.5 h-3.5 text-[var(--accent)]" />
              <input
                type="text"
                value={findText}
                onChange={(e) => {
                  setFindText(e.target.value);
                  const wv = webviewRefs.current[activeTabId];
                  if (wv && typeof wv.findInPage === 'function' && e.target.value) {
                    wv.findInPage(e.target.value, { forward: true, findNext: false });
                  }
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') runFindInPage(!e.shiftKey);
                }}
                placeholder="Rechercher dans la page…"
                className="bg-transparent text-[11.5px] text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none w-44"
                autoFocus
              />
              {findResults !== null && (
                <span className="text-[10px] tabular-nums text-[var(--text-3)] shrink-0">
                  {findResults} résultat(s)
                </span>
              )}
              <button
                type="button"
                onClick={() => runFindInPage(true)}
                className="btn-glass !py-1 !px-2 !text-[10px]"
              >
                Suivant
              </button>
              <button
                type="button"
                onClick={() => {
                  setShowFindBar(false);
                  webviewRefs.current[activeTabId]?.stopFindInPage('clearSelection');
                }}
                className="icon-btn w-6 h-6"
              >
                <X className="w-3 h-3" />
              </button>
            </div>
          )}

          {/* Barre d'état Aurora */}
          {!zenMode && (
            <footer className="glass-thin h-6 shrink-0 border-t border-[var(--stroke)] flex items-center gap-3 px-3 text-[9.5px] text-[var(--text-3)] tabular-nums">
              <span className="flex items-center gap-1 text-emerald-400">
                <Shield className="w-2.5 h-2.5" />
                Shield {shieldStats.enabled ? 'ON' : 'OFF'}
              </span>
              <span>{shieldStats.totalBlocked} bloqués</span>
              <span className="opacity-40">·</span>
              <span>{vitals.memoryMb ? `${vitals.memoryMb} Mo` : 'RAM —'}</span>
              <span>{vitals.cpuPercent}% CPU</span>
              <span className="opacity-40">·</span>
              <span>{tabs.length} onglets · {visibleTabs.length} dans l’espace</span>
              <span className="ml-auto flex items-center gap-1 max-w-[320px] truncate">
                {activeTab?.url !== NEWTAB ? prettyUrl(activeTab.url, 46) : 'Orakle Aurora — prêt'}
              </span>
              <span className="flex items-center gap-1">
                <Moon className="w-2.5 h-2.5" />
                v1.1
              </span>
            </footer>
          )}
        </div>

        {/* Panneau latéral global */}
        <SidePanel
          activeTab={sidePanel}
          onClose={() => setSidePanel(null)}
          onSelectPanel={setSidePanel}
          onNavigate={(url) => navigateTab(activeTabId, url)}
          bookmarks={bookmarks}
          onAddBookmark={(bm) => window.orakle?.storage.addBookmark(bm).then(setBookmarks)}
          onRemoveBookmark={(id) => window.orakle?.storage.removeBookmark(id).then(setBookmarks)}
          history={history}
          onClearHistory={() => window.orakle?.storage.clearHistory().then(setHistory)}
          onRemoveHistoryItem={(id) => window.orakle?.storage.removeHistoryItem(id).then(setHistory)}
          downloads={downloads}
          scratchpad={scratchpad}
          onUpdateScratchpad={(val) => {
            setScratchpad(val);
            window.orakle?.storage.updateScratchpad(val);
          }}
          settings={settings}
          onUpdateSettings={handleUpdateSettings}
          onClearBrowsingData={async () => {
            await window.orakle?.security.clearBrowsingData();
            setHistory([]);
            notify('Données de navigation purgées', 'ok');
          }}
          currentUrl={activeTab?.url || ''}
          tabs={visibleTabs}
          sessions={sessions}
          onSaveSession={handleSaveSessionFromPanel}
          onDeleteSession={handleDeleteSession}
          onRestoreSession={handleRestoreSession}
          onOpenUrls={handleOpenUrls}
          spaces={spaces}
          onSaveSpaces={handleSaveSpaces}
          onOpenReader={() => setReaderOpen(true)}
        />
      </div>

      {/* Bouton de sortie du mode Zen */}
      {zenMode && (
        <button
          type="button"
          onClick={() => setZenMode(false)}
          className="fixed top-3 right-3 z-50 glass-panel-strong px-3 py-1.5 rounded-full text-[10.5px] font-bold flex items-center gap-1.5 opacity-25 hover:opacity-100 transition-opacity cursor-pointer"
        >
          <ShieldOff className="w-3 h-3" />
          Quitter Zen (F11)
        </button>
      )}

      {/* Toast */}
      {toast && (
        <div className="fixed bottom-10 left-1/2 -translate-x-1/2 z-[80] glass-panel-strong px-4 py-2 rounded-full text-[11.5px] font-semibold flex items-center gap-2 animate-pop">
          <span
            className="w-1.5 h-1.5 rounded-full"
            style={{
              background:
                toast.tone === 'ok' ? 'var(--ok)' : toast.tone === 'warn' ? 'var(--warn)' : 'var(--accent)',
            }}
          />
          {toast.text}
        </div>
      )}

      {/* Modales */}
      <ShieldModal
        isOpen={shieldModalOpen}
        onClose={() => setShieldModalOpen(false)}
        currentUrl={activeTab?.url || ''}
        shieldStats={shieldStats}
        settings={settings}
        onUpdateSettings={handleUpdateSettings}
        onToggleHostWhitelist={(host) => window.orakle?.shield.toggleHost(host).then(setShieldStats)}
      />

      <CommandPalette
        isOpen={commandPaletteOpen}
        onClose={() => setCommandPaletteOpen(false)}
        tabs={tabs}
        onSwitchTab={(id) => {
          const t = tabs.find((x) => x.id === id);
          if (t) setActiveSpaceId(t.spaceId);
          setActiveTabId(id);
        }}
        onNewTab={(url, isPrivate) => handleNewTab(url || NEWTAB, isPrivate)}
        onToggleSplitView={handleToggleSplitView}
        onToggleLayout={() =>
          handleUpdateSettings({
            layoutMode: settings.layoutMode === 'horizontal' ? 'vertical' : 'horizontal',
          })
        }
        onToggleTheme={() =>
          handleUpdateSettings({ theme: settings.theme === 'dark' ? 'light' : 'dark' })
        }
        onOpenShield={() => setShieldModalOpen(true)}
        onOpenDevTools={handleOpenDevTools}
        onClearData={async () => {
          await window.orakle?.security.clearBrowsingData();
          setHistory([]);
          notify('Données purgées', 'ok');
        }}
        onOpenSidePanel={(p) => setSidePanel(p as SidePanelTab)}
        onOpenReader={() => setReaderOpen(true)}
        onOpenFocus={() => setFocusOpen(true)}
        onSaveSession={() => snapshotSession()}
        onToggleZen={() => setZenMode((z) => !z)}
        isDark={settings.theme === 'dark'}
      />

      <ReaderView
        isOpen={readerOpen && canOpenReader}
        url={activeTab?.url || ''}
        settings={settings}
        onUpdateSettings={handleUpdateSettings}
        onClose={() => setReaderOpen(false)}
        onNavigate={(url) => {
          setReaderOpen(false);
          navigateTab(activeTabId, url);
        }}
      />

      <FocusMode
        isOpen={focusOpen}
        onClose={() => setFocusOpen(false)}
        blockList={settings.focusBlockList}
        currentHost={extractHostname(activeTab?.url || '')}
      />
    </div>
  );

  /* ---------------- Rendu des viewports webview ---------------- */
  function renderViewports() {
    return tabs
      .filter((tab) => tab.spaceId === activeSpaceId)
      .map((tab) => {
        const isPrimary = tab.id === activeTabId;
        const isSecondary = splitTabId !== null && tab.id === splitTabId;
        const isVisible = isPrimary || isSecondary;
        const isNewTab = tab.url === NEWTAB;
        const initialSrc = initialWebviewUrlRef.current[tab.id] || (!isNewTab ? tab.url : 'about:blank');

        return (
          <div
            key={tab.id}
            onClick={() => {
              if (splitTabId && activeTabId !== tab.id) setActiveTabId(tab.id);
            }}
            className={`relative h-full rounded-[18px] overflow-hidden transition-all ${
              isVisible ? 'flex-1 flex flex-col' : 'hidden'
            } ${
              splitTabId && isPrimary
                ? 'ring-1 ring-[color-mix(in_srgb,var(--accent)_60%,transparent)] shadow-[0_0_0_1px_var(--accent-soft),0_20px_60px_-30px_var(--accent-glow)]'
                : 'ring-1 ring-[var(--stroke)]'
            }`}
          >
            {splitTabId && isVisible && (
              <div className="px-3 py-1 bg-[var(--bg-elev)] border-b border-[var(--stroke)] flex items-center justify-between text-[10.5px]">
                <span className="font-semibold truncate flex items-center gap-1.5">
                  <span
                    className={`w-1.5 h-1.5 rounded-full ${
                      isPrimary ? 'bg-[var(--accent)]' : 'bg-[var(--text-3)]'
                    }`}
                  />
                  {tab.title}
                </span>
                {isSecondary && (
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      setSplitTabId(null);
                    }}
                    className="text-[var(--text-3)] hover:text-[var(--text)] cursor-pointer"
                    title="Fermer la vue scindée"
                  >
                    <X className="w-3 h-3" />
                  </button>
                )}
              </div>
            )}

            {tab.isSuspended && !isNewTab ? (
              <div className="flex-1 flex flex-col items-center justify-center gap-3 bg-[var(--bg)] text-center px-6">
                <div className="w-12 h-12 rounded-2xl bg-[var(--fill)] border border-[var(--stroke)] flex items-center justify-center">
                  <Moon className="w-5 h-5 text-[var(--accent)]" />
                </div>
                <div>
                  <p className="text-[12.5px] font-semibold">Onglet en veille</p>
                  <p className="text-[10.5px] text-[var(--text-3)] mt-1 max-w-[280px]">
                    Le processus de rendu a été libéré. Cliquez pour recharger {prettyUrl(tab.url, 34)}.
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() =>
                    setTabs((prev) => prev.map((t) => (t.id === tab.id ? { ...t, isSuspended: false } : t)))
                  }
                  className="btn-primary !py-1.5 !text-[11px]"
                >
                  <RotateCw className="w-3 h-3" />
                  Recharger l’onglet
                </button>
              </div>
            ) : isNewTab ? (
              <div className="w-full flex-1 overflow-hidden">
                <NewTabPage
                  onNavigate={(url) => navigateTab(tab.id, url)}
                  searchEngine={settings.searchEngine}
                  onChangeSearchEngine={(eng) => handleUpdateSettings({ searchEngine: eng })}
                  speedDials={speedDials}
                  onUpdateSpeedDials={(dials) => {
                    setSpeedDials(dials);
                    window.orakle?.storage.updateSpeedDials(dials);
                  }}
                  shieldStats={shieldStats}
                  onOpenShieldModal={() => setShieldModalOpen(true)}
                  recentHistory={history}
                  scratchpad={scratchpad}
                  onUpdateScratchpad={(val) => {
                    setScratchpad(val);
                    window.orakle?.storage.updateScratchpad(val);
                  }}
                  isPrivate={tab.isPrivate}
                  spaces={spaces}
                  activeSpaceId={activeSpaceId}
                  sessions={sessions}
                  onRestoreSession={handleRestoreSession}
                  onOpenFocus={() => setFocusOpen(true)}
                  tabCount={visibleTabs.length}
                />
              </div>
            ) : (
              <webview
                ref={(el) => registerWebview(tab.id, tab.isPrivate, el as WebviewElement | null)}
                src={initialSrc}
                partition={tab.isPrivate ? 'orakle-incognito' : 'persist:orakle'}
                allowpopups={true as unknown as string}
                className="w-full flex-1 bg-white"
              />
            )}
          </div>
        );
      });
  }
};
