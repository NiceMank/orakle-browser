import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
  ArrowLeft,
  ArrowRight,
  RotateCw,
  Home,
  Plus,
  X,
  Shield,
  ShieldCheck,
  ShieldAlert,
  Lock,
  Unlock,
  Star,
  Bookmark,
  Clock,
  Download,
  FileText,
  Settings,
  Columns,
  Command,
  Search,
  Volume2,
  VolumeX,
  Pin,
  Maximize2,
  Minimize2,
  Minus,
  Square,
  Terminal,
  ZoomIn,
  ZoomOut,
  EyeOff,
  Compass,
  ChevronRight,
  Sparkles,
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
} from './types/browser';
import {
  resolveOmniboxInput,
  getSecurityState,
  extractHostname,
  SEARCH_ENGINES,
} from './utils/url';
import { NewTabPage } from './components/NewTabPage';
import { ShieldModal } from './components/ShieldModal';
import { SidePanel } from './components/SidePanel';
import { CommandPalette } from './components/CommandPalette';

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

const createTabObject = (url = 'orakle://newtab', isPrivate = false): Tab => ({
  id: `tab-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
  title: url === 'orakle://newtab' ? 'Nouvel Onglet' : extractHostname(url),
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
});

export const App: React.FC = () => {
  // Onglets & Navigation
  const [tabs, setTabs] = useState<Tab[]>(() => [createTabObject('orakle://newtab', false)]);
  const [activeTabId, setActiveTabId] = useState<string>(() => tabs[0].id);
  const [splitTabId, setSplitTabId] = useState<string | null>(null);
  const [closedTabsStack, setClosedTabsStack] = useState<{ url: string; isPrivate: boolean }[]>([]);

  // Omnibox
  const [omniboxText, setOmniboxText] = useState('');
  const [omniboxFocused, setOmniboxFocused] = useState(false);

  // Recherche dans la page (Ctrl+F)
  const [showFindBar, setShowFindBar] = useState(false);
  const [findText, setFindText] = useState('');

  // Mode Zen (Plein écran sans chrome)
  const [zenMode, setZenMode] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);

  // Panneaux et Modales
  const [sidePanel, setSidePanel] = useState<SidePanelTab>(null);
  const [shieldModalOpen, setShieldModalOpen] = useState(false);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);

  // Données persistantes & Paramètres
  const [settings, setSettings] = useState<BrowserSettings>({
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
  });
  const [bookmarks, setBookmarks] = useState<BookmarkItem[]>([]);
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [speedDials, setSpeedDials] = useState<SpeedDialItem[]>([]);
  const [scratchpad, setScratchpad] = useState('');
  const [downloads, setDownloads] = useState<DownloadItemInfo[]>([]);
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

  // Références aux éléments <webview> et URLs initiales
  const webviewRefs = useRef<Record<string, WebviewElement | null>>({});
  const initialWebviewUrlRef = useRef<Record<string, string>>({});
  const attachedListenersRef = useRef<Set<string>>(new Set());
  const omniboxInputRef = useRef<HTMLInputElement | null>(null);

  const activeTab = tabs.find((t) => t.id === activeTabId) || tabs[0];

  // Synchronisation du thème et de la couleur d'accentuation sur le DOM
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', settings.theme);
    document.body.setAttribute('data-accent', settings.accent);
  }, [settings.theme, settings.accent]);

  // Synchronisation de l'Omnibox lors du changement d'onglet actif
  useEffect(() => {
    if (!activeTab) return;
    setOmniboxText(activeTab.url === 'orakle://newtab' ? '' : activeTab.url);
  }, [activeTab?.id, activeTab?.url]);

  // Chargement initial depuis le processus Electron principal
  useEffect(() => {
    if (!window.orakle) return;

    window.orakle.storage.getAll().then((data) => {
      setSettings(data.settings);
      setBookmarks(data.bookmarks);
      setHistory(data.history);
      setSpeedDials(data.speedDials);
      setScratchpad(data.scratchpad);
    });

    window.orakle.shield.getStats().then(setShieldStats);
    window.orakle.downloads.getAll().then(setDownloads);
    window.orakle.window.isMaximized().then(setIsMaximized);

    const unsubShield = window.orakle.shield.onUpdated(setShieldStats);
    const unsubDownloads = window.orakle.downloads.onUpdated(setDownloads);
    const unsubMax = window.orakle.window.onMaximizedChange(setIsMaximized);

    return () => {
      unsubShield();
      unsubDownloads();
      unsubMax();
    };
  }, []);

  // Gestion de la navigation dans un onglet
  const navigateTab = useCallback(
    (tabId: string, rawInput: string) => {
      const targetUrl = resolveOmniboxInput(rawInput, settings.searchEngine);

      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== tabId) return t;
          return {
            ...t,
            url: targetUrl,
            title:
              targetUrl === 'orakle://newtab' ? 'Nouvel Onglet' : extractHostname(targetUrl),
            securityState: getSecurityState(targetUrl),
          };
        })
      );

      if (targetUrl !== 'orakle://newtab') {
        const wv = webviewRefs.current[tabId];
        if (wv && typeof wv.loadURL === 'function') {
          wv.loadURL(targetUrl).catch(() => {});
        } else {
          initialWebviewUrlRef.current[tabId] = targetUrl;
        }
      }
    },
    [settings.searchEngine]
  );

  // Création d'un nouvel onglet
  const handleNewTab = useCallback((url = 'orakle://newtab', isPrivate = false, background = false) => {
    const newTab = createTabObject(url, isPrivate);
    if (url !== 'orakle://newtab') {
      initialWebviewUrlRef.current[newTab.id] = url;
    }
    setTabs((prev) => [...prev, newTab]);
    if (!background) {
      setActiveTabId(newTab.id);
    }
  }, []);

  // Écoute des demandes d'ouverture d'onglet provenant du Main Process (target="_blank", menu contextuel)
  useEffect(() => {
    if (!window.orakle) return;
    return window.orakle.tabs.onOpenFromMain(({ url, background, isPrivate }) => {
      handleNewTab(url, Boolean(isPrivate), Boolean(background));
    });
  }, [handleNewTab]);

  // Fermeture d'un onglet
  const handleCloseTab = useCallback(
    (tabId: string, e?: React.MouseEvent) => {
      e?.stopPropagation();
      const target = tabs.find((t) => t.id === tabId);
      if (target && target.url !== 'orakle://newtab') {
        setClosedTabsStack((prev) => [
          { url: target.url, isPrivate: target.isPrivate },
          ...prev.slice(0, 19),
        ]);
      }

      delete webviewRefs.current[tabId];
      delete initialWebviewUrlRef.current[tabId];
      attachedListenersRef.current.delete(tabId);

      if (splitTabId === tabId) {
        setSplitTabId(null);
      }

      if (tabs.length === 1) {
        const fresh = createTabObject('orakle://newtab', false);
        setTabs([fresh]);
        setActiveTabId(fresh.id);
        return;
      }

      const idx = tabs.findIndex((t) => t.id === tabId);
      const nextTabs = tabs.filter((t) => t.id !== tabId);
      setTabs(nextTabs);

      if (activeTabId === tabId) {
        const fallback = nextTabs[Math.max(0, idx - 1)] || nextTabs[0];
        setActiveTabId(fallback.id);
      }
    },
    [tabs, activeTabId, splitTabId]
  );

  // Attachement des événements sur chaque élément <webview>
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

          if (
            !isPrivate &&
            currentUrl &&
            !currentUrl.startsWith('orakle://') &&
            currentUrl !== 'about:blank'
          ) {
            window.orakle?.storage
              .addHistory({
                title: currentTitle || extractHostname(currentUrl),
                url: currentUrl,
              })
              .then(setHistory);
          }
        } catch {
          // Ignore webview not ready errors
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

      el.addEventListener('page-title-updated', (ev: Event) => {
        const customEv = ev as CustomEvent & { title?: string };
        if (customEv.title) {
          setTabs((prev) =>
            prev.map((t) => (t.id === tabId ? { ...t, title: customEv.title! } : t))
          );
        }
      });

      el.addEventListener('page-favicon-updated', (ev: Event) => {
        const customEv = ev as CustomEvent & { favicons?: string[] };
        if (customEv.favicons && customEv.favicons.length > 0) {
          setTabs((prev) =>
            prev.map((t) => (t.id === tabId ? { ...t, favicon: customEv.favicons![0] } : t))
          );
        }
      });

      el.addEventListener('media-started-playing', () => {
        setTabs((prev) =>
          prev.map((t) => (t.id === tabId ? { ...t, isAudioPlaying: true } : t))
        );
      });

      el.addEventListener('media-paused', () => {
        setTabs((prev) =>
          prev.map((t) => (t.id === tabId ? { ...t, isAudioPlaying: false } : t))
        );
      });
    },
    []
  );

  // Actions de navigation sur l'onglet actif
  const handleGoBack = () => {
    const wv = webviewRefs.current[activeTabId];
    if (wv && wv.canGoBack()) wv.goBack();
  };

  const handleGoForward = () => {
    const wv = webviewRefs.current[activeTabId];
    if (wv && wv.canGoForward()) wv.goForward();
  };

  const handleReload = () => {
    const wv = webviewRefs.current[activeTabId];
    if (wv) {
      if (activeTab.isLoading) wv.stop();
      else wv.reload();
    }
  };

  const handleHome = () => {
    navigateTab(activeTabId, settings.homeUrl || 'orakle://newtab');
  };

  const handleToggleMute = (tabId: string, e?: React.MouseEvent) => {
    e?.stopPropagation();
    const wv = webviewRefs.current[tabId];
    setTabs((prev) =>
      prev.map((t) => {
        if (t.id !== tabId) return t;
        const nextMuted = !t.isMuted;
        if (wv && typeof wv.setAudioMuted === 'function') {
          wv.setAudioMuted(nextMuted);
        }
        return { ...t, isMuted: nextMuted };
      })
    );
  };

  const handleTogglePin = (tabId: string, e?: React.MouseEvent) => {
    e?.stopPropagation();
    setTabs((prev) =>
      prev.map((t) => (t.id === tabId ? { ...t, isPinned: !t.isPinned } : t))
    );
  };

  const handleZoom = (delta: number) => {
    const wv = webviewRefs.current[activeTabId];
    setTabs((prev) =>
      prev.map((t) => {
        if (t.id !== activeTabId) return t;
        const nextZoom =
          delta === 0 ? 1 : Math.min(2.5, Math.max(0.5, +(t.zoomFactor + delta).toFixed(2)));
        if (wv && typeof wv.setZoomFactor === 'function') {
          wv.setZoomFactor(nextZoom);
        }
        return { ...t, zoomFactor: nextZoom };
      })
    );
  };

  const handleToggleSplitView = useCallback(() => {
    if (splitTabId) {
      setSplitTabId(null);
      return;
    }
    const otherTab = tabs.find((t) => t.id !== activeTabId);
    if (otherTab) {
      setSplitTabId(otherTab.id);
    } else {
      const secondTab = createTabObject('https://fr.wikipedia.org', false);
      initialWebviewUrlRef.current[secondTab.id] = secondTab.url;
      setTabs((prev) => [...prev, secondTab]);
      setSplitTabId(secondTab.id);
    }
  }, [splitTabId, tabs, activeTabId]);

  const handleOpenDevTools = useCallback(() => {
    const wv = webviewRefs.current[activeTabId];
    if (wv && typeof wv.openDevTools === 'function') {
      wv.openDevTools();
    }
  }, [activeTabId]);

  // Mise à jour des paramètres
  const handleUpdateSettings = useCallback((patch: Partial<BrowserSettings>) => {
    setSettings((prev) => ({ ...prev, ...patch }));
    window.orakle?.storage.updateSettings(patch).then(setSettings);
  }, []);

  // Gestion des favoris
  const isCurrentBookmarked = bookmarks.some((b) => b.url === activeTab?.url);
  const handleToggleBookmarkCurrent = () => {
    if (!activeTab || activeTab.url.startsWith('orakle://')) return;
    if (isCurrentBookmarked) {
      window.orakle?.storage.removeBookmark(activeTab.url).then(setBookmarks);
    } else {
      window.orakle?.storage
        .addBookmark({
          title: activeTab.title || extractHostname(activeTab.url),
          url: activeTab.url,
          category: 'Favoris',
        })
        .then(setBookmarks);
    }
  };

  // Raccourcis clavier globaux
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 't' && !e.shiftKey) {
        e.preventDefault();
        handleNewTab('orakle://newtab', false);
      } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 't') {
        e.preventDefault();
        if (closedTabsStack.length > 0) {
          const [last, ...rest] = closedTabsStack;
          setClosedTabsStack(rest);
          handleNewTab(last.url, last.isPrivate);
        }
      } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'n') {
        e.preventDefault();
        handleNewTab('orakle://newtab', true);
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'w') {
        e.preventDefault();
        handleCloseTab(activeTabId);
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setCommandPaletteOpen((v) => !v);
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'l') {
        e.preventDefault();
        omniboxInputRef.current?.focus();
        omniboxInputRef.current?.select();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        setShowFindBar((v) => !v);
      } else if ((e.ctrlKey || e.metaKey) && e.key === '\\') {
        e.preventDefault();
        handleToggleSplitView();
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'b') {
        e.preventDefault();
        setSidePanel((p) => (p === 'bookmarks' ? null : 'bookmarks'));
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'h') {
        e.preventDefault();
        setSidePanel((p) => (p === 'history' ? null : 'history'));
      } else if (e.key === 'F12') {
        e.preventDefault();
        handleOpenDevTools();
      } else if (e.key === 'F11') {
        e.preventDefault();
        setZenMode((z) => !z);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [
    activeTabId,
    closedTabsStack,
    handleNewTab,
    handleCloseTab,
    handleToggleSplitView,
    handleOpenDevTools,
  ]);

  // Exécution de la recherche dans la page
  const runFindInPage = (forward = true) => {
    if (!findText.trim()) return;
    const wv = webviewRefs.current[activeTabId];
    if (wv && typeof wv.findInPage === 'function') {
      wv.findInPage(findText, { forward, findNext: true });
    }
  };

  // Rendu d'un onglet (utilisé en mode Horizontal ou Vertical)
  const renderTabItem = (tab: Tab, vertical = false) => {
    const isActive = tab.id === activeTabId;
    const isSplit = tab.id === splitTabId;

    return (
      <div
        key={tab.id}
        onClick={() => setActiveTabId(tab.id)}
        onDoubleClick={() => handleTogglePin(tab.id)}
        className={`app-no-drag group relative flex items-center gap-2 transition-all cursor-pointer select-none ${
          vertical
            ? 'w-full px-3 py-2 rounded-xl'
            : tab.isPinned
            ? 'w-10 justify-center py-1.5 rounded-xl shrink-0'
            : 'min-w-[130px] max-w-[220px] flex-1 px-3 py-1.5 rounded-xl'
        } ${
          isActive
            ? 'bg-[var(--bg-elev)] text-[var(--text)] border border-[var(--stroke-strong)] shadow-sm'
            : isSplit
            ? 'bg-[var(--accent-soft)] text-[var(--text)] border border-[var(--accent)]/40'
            : 'text-[var(--text-2)] hover:text-[var(--text)] hover:bg-[var(--fill)] border border-transparent'
        }`}
        title={`${tab.title}\nDouble-clic pour épingler`}
      >
        {/* Indicateur actif */}
        {isActive && (
          <span
            className={
              vertical
                ? 'absolute left-0 top-2 bottom-2 w-1 rounded-r-full'
                : 'absolute bottom-0 left-3 right-3 h-0.5 rounded-t-full'
            }
            style={{
              backgroundColor: tab.isPrivate ? '#a855f7' : 'var(--accent)',
            }}
          />
        )}

        {/* Icône / Favicon / Spinner */}
        {tab.isLoading ? (
          <RotateCw className="w-3.5 h-3.5 animate-spin text-[var(--accent)] shrink-0" />
        ) : tab.isPrivate ? (
          <EyeOff className="w-3.5 h-3.5 text-purple-400 shrink-0" />
        ) : tab.favicon ? (
          <img
            src={tab.favicon}
            alt=""
            className="w-3.5 h-3.5 rounded-xs shrink-0 object-contain"
            onError={(e) => {
              (e.target as HTMLImageElement).style.display = 'none';
            }}
          />
        ) : (
          <Compass className="w-3.5 h-3.5 text-[var(--accent)] shrink-0" />
        )}

        {/* Titre de l'onglet */}
        {(!tab.isPinned || vertical) && (
          <span className="text-xs font-medium truncate flex-1">{tab.title}</span>
        )}

        {/* Contrôle Audio si lecture en cours */}
        {(tab.isAudioPlaying || tab.isMuted) && (
          <button
            type="button"
            onClick={(e) => handleToggleMute(tab.id, e)}
            className="p-0.5 rounded-md hover:bg-[var(--fill-strong)] text-[var(--accent)]"
            title={tab.isMuted ? 'Réactiver le son' : 'Couper le son de cet onglet'}
          >
            {tab.isMuted ? (
              <VolumeX className="w-3 h-3 text-red-400" />
            ) : (
              <Volume2 className="w-3 h-3" />
            )}
          </button>
        )}

        {/* Épingle */}
        {tab.isPinned && vertical && <Pin className="w-3 h-3 text-[var(--accent)] shrink-0" />}

        {/* Bouton Fermer */}
        {!tab.isPinned && (
          <button
            type="button"
            onClick={(e) => handleCloseTab(tab.id, e)}
            className="opacity-0 group-hover:opacity-100 p-0.5 rounded-md hover:bg-[var(--fill-strong)] text-[var(--text-3)] hover:text-[var(--text)] transition-opacity cursor-pointer shrink-0"
            aria-label={`Fermer ${tab.title}`}
          >
            <X className="w-3 h-3" />
          </button>
        )}
      </div>
    );
  };

  return (
    <div className="flex flex-col w-screen h-screen overflow-hidden bg-[var(--bg)] text-[var(--text)]">
      {/* Barre supérieure (Masquée uniquement en mode Zen F11) */}
      {!zenMode && (
        <header className="glass-panel z-30 flex flex-col border-b border-[var(--stroke)] shrink-0">
          {/* Ligne 1 : Barre d'onglets horizontale + Contrôles de fenêtre */}
          {settings.layoutMode === 'horizontal' && (
            <div className="app-drag flex items-center justify-between gap-2 px-3 pt-2 pb-1">
              {/* Identité Orakle */}
              <div className="app-no-drag flex items-center gap-2 pr-1">
                <div
                  className="w-7 h-7 rounded-xl flex items-center justify-center text-white font-black text-xs shadow-md"
                  style={{
                    background:
                      'linear-gradient(135deg, var(--accent) 0%, color-mix(in srgb, var(--accent) 60%, #8b5cf6) 100%)',
                  }}
                  title="Orakle Browser"
                >
                  O
                </div>
              </div>

              {/* Liste des onglets */}
              <div className="flex items-center gap-1.5 flex-1 min-w-0 overflow-x-auto no-scrollbar">
                {tabs.map((tab) => renderTabItem(tab, false))}

                <button
                  type="button"
                  onClick={() => handleNewTab('orakle://newtab', false)}
                  className="app-no-drag icon-btn w-7 h-7 shrink-0"
                  title="Nouvel Onglet (Ctrl+T)"
                  aria-label="Nouvel Onglet"
                >
                  <Plus className="w-4 h-4" />
                </button>

                <button
                  type="button"
                  onClick={() => handleNewTab('orakle://newtab', true)}
                  className="app-no-drag icon-btn w-7 h-7 shrink-0 text-purple-400 hover:text-purple-300"
                  title="Nouvel Onglet Fantôme Privé (Ctrl+Shift+N)"
                  aria-label="Nouvel Onglet Privé"
                >
                  <EyeOff className="w-3.5 h-3.5" />
                </button>
              </div>

              {/* Boutons de contrôle de fenêtre natifs */}
              <div className="app-no-drag flex items-center gap-1 pl-2">
                <button
                  type="button"
                  onClick={() => window.orakle?.window.minimize()}
                  className="icon-btn w-7 h-7"
                  title="Réduire"
                  aria-label="Réduire la fenêtre"
                >
                  <Minus className="w-3.5 h-3.5" />
                </button>
                <button
                  type="button"
                  onClick={() => window.orakle?.window.maximize()}
                  className="icon-btn w-7 h-7"
                  title={isMaximized ? 'Restaurer' : 'Agrandir'}
                  aria-label="Agrandir la fenêtre"
                >
                  <Square className="w-3 h-3" />
                </button>
                <button
                  type="button"
                  onClick={() => window.orakle?.window.close()}
                  className="icon-btn w-7 h-7 hover:bg-red-500/20 hover:text-red-400"
                  title="Fermer Orakle Browser"
                  aria-label="Fermer la fenêtre"
                >
                  <X className="w-4 h-4" />
                </button>
              </div>
            </div>
          )}

          {/* Ligne 2 : Barre de Navigation & Omnibox Intelligente */}
          <div className="app-drag flex items-center gap-2 px-3 py-2">
            {settings.layoutMode === 'vertical' && (
              <div className="app-no-drag flex items-center gap-2 pr-1">
                <div
                  className="w-7 h-7 rounded-xl flex items-center justify-center text-white font-black text-xs shadow-md"
                  style={{ backgroundColor: 'var(--accent)' }}
                >
                  O
                </div>
              </div>
            )}

            {/* Boutons Précédent / Suivant / Actualiser / Accueil */}
            <div className="app-no-drag flex items-center gap-1">
              <button
                type="button"
                onClick={handleGoBack}
                disabled={!activeTab?.canGoBack}
                className="icon-btn w-8 h-8"
                title="Page précédente"
                aria-label="Page précédente"
              >
                <ArrowLeft className="w-4 h-4" />
              </button>
              <button
                type="button"
                onClick={handleGoForward}
                disabled={!activeTab?.canGoForward}
                className="icon-btn w-8 h-8"
                title="Page suivante"
                aria-label="Page suivante"
              >
                <ArrowRight className="w-4 h-4" />
              </button>
              <button
                type="button"
                onClick={handleReload}
                className="icon-btn w-8 h-8"
                title="Actualiser la page (Ctrl+R)"
                aria-label="Actualiser la page"
              >
                <RotateCw
                  className={`w-4 h-4 ${activeTab?.isLoading ? 'animate-spin text-[var(--accent)]' : ''}`}
                />
              </button>
              <button
                type="button"
                onClick={handleHome}
                className="icon-btn w-8 h-8"
                title="Accueil Orakle"
                aria-label="Accueil"
              >
                <Home className="w-4 h-4" />
              </button>
            </div>

            {/* Omnibox Centrale */}
            <form
              onSubmit={(e) => {
                e.preventDefault();
                navigateTab(activeTabId, omniboxText);
                omniboxInputRef.current?.blur();
              }}
              className={`app-no-drag flex-1 max-w-3xl mx-auto flex items-center gap-2.5 px-3.5 py-1.5 rounded-2xl border transition-all ${
                omniboxFocused
                  ? 'bg-[var(--bg-elev)] border-[var(--accent)] shadow-[0_0_0_3px_var(--accent-soft)]'
                  : 'bg-[var(--fill)] border-[var(--stroke)] hover:border-[var(--stroke-strong)]'
              }`}
            >
              {/* Statut SSL / Sécurité de l'URL */}
              <button
                type="button"
                onClick={() => setShieldModalOpen(true)}
                className="flex items-center gap-1 text-xs font-medium px-1.5 py-0.5 rounded-lg hover:bg-[var(--fill-strong)] cursor-pointer shrink-0"
                title="Inspecter la sécurité de la connexion et Orakle Shield"
              >
                {activeTab?.securityState === 'secure' ? (
                  <Lock className="w-3.5 h-3.5 text-emerald-400" />
                ) : activeTab?.securityState === 'internal' ? (
                  <Sparkles className="w-3.5 h-3.5 text-[var(--accent)]" />
                ) : (
                  <Unlock className="w-3.5 h-3.5 text-amber-400" />
                )}
              </button>

              <input
                ref={omniboxInputRef}
                type="text"
                value={omniboxText}
                onFocus={(e) => {
                  setOmniboxFocused(true);
                  e.target.select();
                }}
                onBlur={() => setOmniboxFocused(false)}
                onChange={(e) => setOmniboxText(e.target.value)}
                placeholder={`Rechercher avec ${SEARCH_ENGINES[settings.searchEngine].name} ou saisir une URL (Ctrl+L)...`}
                className="w-full bg-transparent text-xs md:text-sm text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none"
              />

              {/* Indicateur de Zoom si différent de 100% */}
              {activeTab && activeTab.zoomFactor !== 1 && (
                <button
                  type="button"
                  onClick={() => handleZoom(0)}
                  className="px-2 py-0.5 rounded-md text-[10px] font-bold bg-[var(--fill-strong)] text-[var(--text-2)] hover:text-[var(--text)] cursor-pointer shrink-0"
                  title="Réinitialiser le zoom à 100%"
                >
                  {Math.round(activeTab.zoomFactor * 100)}%
                </button>
              )}

              {/* Bouton Favori étoile */}
              {activeTab && !activeTab.url.startsWith('orakle://') && (
                <button
                  type="button"
                  onClick={handleToggleBookmarkCurrent}
                  className="p-1 rounded-lg hover:bg-[var(--fill-strong)] transition-colors cursor-pointer shrink-0"
                  title={
                    isCurrentBookmarked ? 'Retirer des favoris' : 'Ajouter cette page aux favoris'
                  }
                >
                  <Star
                    className={`w-3.5 h-3.5 ${
                      isCurrentBookmarked
                        ? 'fill-amber-400 text-amber-400'
                        : 'text-[var(--text-2)]'
                    }`}
                  />
                </button>
              )}
            </form>

            {/* Actions Rapides à droite de l'Omnibox */}
            <div className="app-no-drag flex items-center gap-1">
              {/* Bouton Bouclier Orakle Shield avec compteur en direct */}
              <button
                type="button"
                onClick={() => setShieldModalOpen((v) => !v)}
                className={`px-2.5 py-1.5 rounded-xl text-xs font-bold flex items-center gap-1.5 border transition-all cursor-pointer ${
                  shieldStats.enabled
                    ? 'bg-emerald-500/12 text-emerald-400 border-emerald-500/25 hover:bg-emerald-500/20'
                    : 'bg-amber-500/12 text-amber-400 border-amber-500/25'
                }`}
                title="Orakle Shield & Audit de Sécurité OWASP"
              >
                {shieldStats.enabled ? (
                  <ShieldCheck className="w-4 h-4" />
                ) : (
                  <ShieldAlert className="w-4 h-4" />
                )}
                <span className="tabular-nums">{shieldStats.totalBlocked}</span>
              </button>

              {/* Écran Scindé (Split View) */}
              <button
                type="button"
                onClick={handleToggleSplitView}
                className={`icon-btn w-8 h-8 ${
                  splitTabId ? 'text-[var(--accent)] bg-[var(--accent-soft)]' : ''
                }`}
                title="Mode Écran Scindé / Split View (Ctrl+\)"
                aria-label="Écran scindé"
              >
                <Columns className="w-4 h-4" />
              </button>

              {/* Recherche dans la page */}
              <button
                type="button"
                onClick={() => setShowFindBar((v) => !v)}
                className={`icon-btn w-8 h-8 ${
                  showFindBar ? 'text-[var(--accent)] bg-[var(--accent-soft)]' : ''
                }`}
                title="Rechercher dans la page (Ctrl+F)"
                aria-label="Rechercher dans la page"
              >
                <Search className="w-4 h-4" />
              </button>

              {/* Zoom - / + */}
              <button
                type="button"
                onClick={() => handleZoom(-0.1)}
                className="icon-btn w-8 h-8 hidden sm:inline-flex"
                title="Zoom arrière"
                aria-label="Zoom arrière"
              >
                <ZoomOut className="w-4 h-4" />
              </button>
              <button
                type="button"
                onClick={() => handleZoom(0.1)}
                className="icon-btn w-8 h-8 hidden sm:inline-flex"
                title="Zoom avant"
                aria-label="Zoom avant"
              >
                <ZoomIn className="w-4 h-4" />
              </button>

              {/* Palette de commandes Ctrl+K */}
              <button
                type="button"
                onClick={() => setCommandPaletteOpen(true)}
                className="icon-btn w-8 h-8"
                title="Palette de Commandes (Ctrl+K)"
                aria-label="Palette de commandes"
              >
                <Command className="w-4 h-4" />
              </button>

              {/* Accès aux panneaux latéraux */}
              <button
                type="button"
                onClick={() => setSidePanel((p) => (p === 'bookmarks' ? null : 'bookmarks'))}
                className={`icon-btn w-8 h-8 ${
                  sidePanel === 'bookmarks' ? 'text-[var(--accent)] bg-[var(--accent-soft)]' : ''
                }`}
                title="Favoris (Ctrl+B)"
                aria-label="Favoris"
              >
                <Bookmark className="w-4 h-4" />
              </button>

              <button
                type="button"
                onClick={() => setSidePanel((p) => (p === 'downloads' ? null : 'downloads'))}
                className={`icon-btn w-8 h-8 relative ${
                  sidePanel === 'downloads' ? 'text-[var(--accent)] bg-[var(--accent-soft)]' : ''
                }`}
                title="Téléchargements"
                aria-label="Téléchargements"
              >
                <Download className="w-4 h-4" />
                {downloads.some((d) => d.state === 'progressing') && (
                  <span className="absolute top-1 right-1 w-2 h-2 rounded-full bg-[var(--accent)] animate-ping" />
                )}
              </button>

              <button
                type="button"
                onClick={() => setSidePanel((p) => (p === 'notes' ? null : 'notes'))}
                className={`icon-btn w-8 h-8 ${
                  sidePanel === 'notes' ? 'text-[var(--accent)] bg-[var(--accent-soft)]' : ''
                }`}
                title="Scratchpad & Notes"
                aria-label="Scratchpad"
              >
                <FileText className="w-4 h-4" />
              </button>

              <button
                type="button"
                onClick={handleOpenDevTools}
                className="icon-btn w-8 h-8"
                title="Inspecteur DevTools (F12)"
                aria-label="DevTools"
              >
                <Terminal className="w-4 h-4" />
              </button>

              <button
                type="button"
                onClick={() => setSidePanel((p) => (p === 'settings' ? null : 'settings'))}
                className={`icon-btn w-8 h-8 ${
                  sidePanel === 'settings' ? 'text-[var(--accent)] bg-[var(--accent-soft)]' : ''
                }`}
                title="Paramètres & Apparence"
                aria-label="Paramètres"
              >
                <Settings className="w-4 h-4" />
              </button>

              {settings.layoutMode === 'vertical' && (
                <div className="flex items-center gap-1 pl-2 border-l border-[var(--stroke)]">
                  <button
                    type="button"
                    onClick={() => window.orakle?.window.minimize()}
                    className="icon-btn w-7 h-7"
                    aria-label="Réduire"
                  >
                    <Minus className="w-3.5 h-3.5" />
                  </button>
                  <button
                    type="button"
                    onClick={() => window.orakle?.window.maximize()}
                    className="icon-btn w-7 h-7"
                    aria-label="Agrandir"
                  >
                    <Square className="w-3 h-3" />
                  </button>
                  <button
                    type="button"
                    onClick={() => window.orakle?.window.close()}
                    className="icon-btn w-7 h-7 hover:bg-red-500/20 hover:text-red-400"
                    aria-label="Fermer"
                  >
                    <X className="w-4 h-4" />
                  </button>
                </div>
              )}
            </div>
          </div>

          {/* Ligne 3 optionnelle : Barre de Favoris Rapide */}
          {settings.showBookmarksBar && bookmarks.length > 0 && (
            <div className="flex items-center gap-1.5 px-3 py-1 border-t border-[var(--stroke)] overflow-x-auto no-scrollbar">
              {bookmarks.map((bm) => (
                <button
                  key={bm.id}
                  type="button"
                  onClick={() => navigateTab(activeTabId, bm.url)}
                  className="px-2.5 py-1 rounded-lg text-[11px] font-medium text-[var(--text-2)] hover:text-[var(--text)] hover:bg-[var(--fill-hover)] flex items-center gap-1.5 shrink-0 transition-colors cursor-pointer"
                  title={bm.url}
                >
                  <span
                    className="w-1.5 h-1.5 rounded-full"
                    style={{ backgroundColor: 'var(--accent)' }}
                  />
                  <span className="truncate max-w-[140px]">{bm.title}</span>
                </button>
              ))}
            </div>
          )}

          {/* Barre de progression fine pendant le chargement */}
          {activeTab?.isLoading && (
            <div className="w-full h-0.5 bg-transparent overflow-hidden">
              <div
                className="w-1/2 h-full loading-bar-indeterminate rounded-full"
                style={{ backgroundColor: 'var(--accent)' }}
              />
            </div>
          )}
        </header>
      )}

      {/* Barre flottante de recherche dans la page (Ctrl+F) */}
      {showFindBar && (
        <div className="fixed top-24 right-8 z-40 glass-panel-strong px-3 py-2 rounded-2xl flex items-center gap-2 shadow-xl">
          <Search className="w-4 h-4 text-[var(--accent)]" />
          <input
            type="text"
            value={findText}
            onChange={(e) => setFindText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') runFindInPage(!e.shiftKey);
              if (e.key === 'Escape') {
                setShowFindBar(false);
                webviewRefs.current[activeTabId]?.stopFindInPage('clearSelection');
              }
            }}
            placeholder="Rechercher dans la page..."
            className="bg-transparent text-xs text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none w-48"
            autoFocus
          />
          <button
            type="button"
            onClick={() => runFindInPage(true)}
            className="px-2 py-1 rounded-lg text-[11px] font-semibold bg-[var(--fill-strong)] hover:bg-[var(--fill-hover)] cursor-pointer"
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
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      {/* Bouton de sortie du mode Zen */}
      {zenMode && (
        <button
          type="button"
          onClick={() => setZenMode(false)}
          className="fixed top-3 right-3 z-50 glass-panel-strong px-3 py-1.5 rounded-full text-xs font-bold flex items-center gap-1.5 opacity-30 hover:opacity-100 transition-opacity cursor-pointer"
        >
          <Minimize2 className="w-3.5 h-3.5" />
          Quitter Mode Zen (F11)
        </button>
      )}

      {/* Corps Principal : Barre Verticale (si active) + Zone Viewport(s) + SidePanel */}
      <div className="flex flex-1 min-h-0 overflow-hidden relative">
        {/* Barre latérale verticale style Arc Browser */}
        {!zenMode && settings.layoutMode === 'vertical' && (
          <aside className="glass-panel w-60 h-full border-r border-[var(--stroke)] p-2.5 flex flex-col justify-between shrink-0 z-20">
            <div className="space-y-1.5 overflow-y-auto flex-1 pr-1">
              <div className="px-2 py-1 text-[10px] font-bold uppercase tracking-wider text-[var(--text-3)] flex items-center justify-between">
                <span>Onglets ({tabs.length})</span>
              </div>
              {tabs.map((tab) => renderTabItem(tab, true))}
            </div>

            <div className="pt-2 border-t border-[var(--stroke)] space-y-1.5">
              <button
                type="button"
                onClick={() => handleNewTab('orakle://newtab', false)}
                className="w-full py-2 px-3 rounded-xl text-xs font-semibold bg-[var(--accent-soft)] text-[var(--accent)] hover:opacity-90 flex items-center justify-center gap-2 cursor-pointer"
              >
                <Plus className="w-4 h-4" />
                Nouvel Onglet
              </button>
              <button
                type="button"
                onClick={() => handleNewTab('orakle://newtab', true)}
                className="w-full py-1.5 px-3 rounded-xl text-xs font-medium bg-purple-500/15 text-purple-300 hover:bg-purple-500/25 flex items-center justify-center gap-2 cursor-pointer"
              >
                <EyeOff className="w-3.5 h-3.5" />
                Onglet Fantôme
              </button>
            </div>
          </aside>
        )}

        {/* Conteneur des pages Web (Supporte l'affichage simple ET l'écran scindé Split View) */}
        <main className="flex-1 h-full flex gap-1.5 p-1.5 bg-[var(--bg)] overflow-hidden">
          {tabs.map((tab) => {
            const isPrimary = tab.id === activeTabId;
            const isSecondary = splitTabId !== null && tab.id === splitTabId;
            const isVisible = isPrimary || isSecondary;
            const isNewTab = tab.url === 'orakle://newtab';
            const initialSrc =
              initialWebviewUrlRef.current[tab.id] ||
              (!isNewTab ? tab.url : 'about:blank');

            return (
              <div
                key={tab.id}
                onClick={() => {
                  if (splitTabId && activeTabId !== tab.id) {
                    setActiveTabId(tab.id);
                  }
                }}
                className={`relative h-full rounded-2xl overflow-hidden border transition-all ${
                  isVisible ? 'flex-1 flex flex-col' : 'hidden'
                } ${
                  splitTabId && isPrimary
                    ? 'border-[var(--accent)] shadow-lg'
                    : 'border-[var(--stroke)]'
                }`}
              >
                {/* En-tête miniature lorsque l'écran scindé (Split View) est actif */}
                {splitTabId && isVisible && (
                  <div className="px-3 py-1 bg-[var(--bg-elev)] border-b border-[var(--stroke)] flex items-center justify-between text-[11px]">
                    <span className="font-semibold truncate flex items-center gap-1.5">
                      <span
                        className={`w-2 h-2 rounded-full ${
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
                        <X className="w-3.5 h-3.5" />
                      </button>
                    )}
                  </div>
                )}

                {/* Contenu : soit la page Nouvel Onglet Orakle, soit le moteur Chromium <webview> */}
                {isNewTab ? (
                  <div className="w-full flex-1 overflow-hidden bg-[var(--bg)]">
                    <NewTabPage
                      onNavigate={(url) => navigateTab(tab.id, url)}
                      searchEngine={settings.searchEngine}
                      onChangeSearchEngine={(eng) =>
                        handleUpdateSettings({ searchEngine: eng })
                      }
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
                    />
                  </div>
                ) : (
                  <webview
                    ref={(el) =>
                      registerWebview(tab.id, tab.isPrivate, el as WebviewElement | null)
                    }
                    src={initialSrc}
                    partition={tab.isPrivate ? 'orakle-incognito' : 'persist:orakle'}
                    allowpopups={true as unknown as string}
                    className="w-full flex-1 bg-white"
                  />
                )}
              </div>
            );
          })}
        </main>

        {/* Panneau latéral (Favoris, Historique, Téléchargements, Notes, Paramètres) */}
        <SidePanel
          activeTab={sidePanel}
          onClose={() => setSidePanel(null)}
          onSelectPanel={setSidePanel}
          onNavigate={(url) => navigateTab(activeTabId, url)}
          bookmarks={bookmarks}
          onAddBookmark={(bm) => {
            window.orakle?.storage.addBookmark(bm).then(setBookmarks);
          }}
          onRemoveBookmark={(id) => {
            window.orakle?.storage.removeBookmark(id).then(setBookmarks);
          }}
          history={history}
          onClearHistory={() => {
            window.orakle?.storage.clearHistory().then(setHistory);
          }}
          onRemoveHistoryItem={(id) => {
            window.orakle?.storage.removeHistoryItem(id).then(setHistory);
          }}
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
          }}
        />
      </div>

      {/* Modale Orakle Shield & Audit OWASP */}
      <ShieldModal
        isOpen={shieldModalOpen}
        onClose={() => setShieldModalOpen(false)}
        currentUrl={activeTab?.url || ''}
        shieldStats={shieldStats}
        settings={settings}
        onUpdateSettings={handleUpdateSettings}
        onToggleHostWhitelist={(host) => {
          window.orakle?.shield.toggleHost(host).then(setShieldStats);
        }}
      />

      {/* Palette de Commandes Rapides (Ctrl+K) */}
      <CommandPalette
        isOpen={commandPaletteOpen}
        onClose={() => setCommandPaletteOpen(false)}
        tabs={tabs}
        onSwitchTab={(id) => setActiveTabId(id)}
        onNewTab={(url, isPrivate) => handleNewTab(url, isPrivate)}
        onToggleSplitView={handleToggleSplitView}
        onToggleLayout={() =>
          handleUpdateSettings({
            layoutMode: settings.layoutMode === 'horizontal' ? 'vertical' : 'horizontal',
          })
        }
        onToggleTheme={() =>
          handleUpdateSettings({
            theme: settings.theme === 'dark' ? 'light' : 'dark',
          })
        }
        onOpenShield={() => setShieldModalOpen(true)}
        onOpenDevTools={handleOpenDevTools}
        onClearData={async () => {
          await window.orakle?.security.clearBrowsingData();
          setHistory([]);
        }}
        onOpenSidePanel={(p) => setSidePanel(p)}
      />
    </div>
  );
};
