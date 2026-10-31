import React, { useState } from 'react';
import {
  ArrowLeft,
  ArrowRight,
  RotateCw,
  Home,
  ShieldCheck,
  ShieldAlert,
  Columns2,
  Command,
  Bookmark,
  Download,
  FileText,
  Settings,
  Terminal,
  Cookie,
  Gauge,
  Layers,
  BookOpen,
  Sparkles,
  Minus,
  Square,
  X,
  ChevronDown,
  MoreHorizontal,
  ZoomIn,
  ZoomOut,
  PanelRight,
  Star,
  Sun,
  Moon,
} from 'lucide-react';
import {
  BookmarkItem,
  BrowserSettings,
  HistoryItem,
  SearchEngineId,
  Tab,
} from '../types/browser';
import { SEARCH_ENGINES, extractHostname, prettyUrl } from '../utils/url';
import { Omnibox } from './Omnibox';
import { TabStrip } from './TabStrip';

interface TopChromeProps {
  tabs: Tab[];
  activeTab: Tab;
  activeTabId: string;
  splitTabId: string | null;
  settings: BrowserSettings;
  omniboxText: string;
  omniboxFocused: boolean;
  omniboxInputRef: React.RefObject<HTMLInputElement>;
  onOmniboxChange: (v: string) => void;
  onOmniboxSubmit: (v: string) => void;
  onOmniboxFocusChange: (f: boolean) => void;
  onSelectTab: (id: string) => void;
  onCloseTab: (id: string, e?: React.MouseEvent) => void;
  onNewTab: (isPrivate?: boolean) => void;
  onToggleMute: (id: string, e?: React.MouseEvent) => void;
  onTogglePin: (id: string, e?: React.MouseEvent) => void;
  onSleepTab: (id: string) => void;
  onSplitWith: (id: string) => void;
  onBack: () => void;
  onForward: () => void;
  onReload: () => void;
  onHome: () => void;
  onStop: () => void;
  onToggleSplit: () => void;
  onShowFind: () => void;
  showFind: boolean;
  onZoom: (delta: number) => void;
  onOpenPalette: () => void;
  onTogglePanel: (panel: 'bookmarks' | 'downloads' | 'notes' | 'settings' | 'privacy' | 'pulse' | 'spaces') => void;
  activePanel: string | null;
  onOpenShield: () => void;
  onOpenReader: () => void;
  onOpenFocus: () => void;
  onToggleBookmark: () => void;
  isBookmarked: boolean;
  canOpenReader: boolean;
  shieldEnabled: boolean;
  shieldBlocked: number;
  downloadsActive: boolean;
  isMaximized: boolean;
  onMinimize: () => void;
  onMaximize: () => void;
  onCloseWindow: () => void;
  onUpdateSettings: (patch: Partial<BrowserSettings>) => void;
  onToggleTheme: () => void;
  bookmarks: BookmarkItem[];
  history: HistoryItem[];
}

const WindowControls: React.FC<{
  isMaximized: boolean;
  onMinimize: () => void;
  onMaximize: () => void;
  onClose: () => void;
}> = ({ isMaximized, onMinimize, onMaximize, onClose }) => (
  <div className="app-no-drag flex items-center gap-0.5 pl-1">
    <button type="button" onClick={onMinimize} className="icon-btn w-7 h-7" title="Réduire" aria-label="Réduire">
      <Minus className="w-3.5 h-3.5" />
    </button>
    <button
      type="button"
      onClick={onMaximize}
      className="icon-btn w-7 h-7"
      title={isMaximized ? 'Restaurer' : 'Agrandir'}
      aria-label="Agrandir"
    >
      <Square className="w-2.5 h-2.5" />
    </button>
    <button
      type="button"
      onClick={onClose}
      className="icon-btn w-7 h-7 hover:!bg-red-500/25 hover:!text-red-400"
      title="Fermer Orakle"
      aria-label="Fermer"
    >
      <X className="w-3.5 h-3.5" />
    </button>
  </div>
);

/**
 * Chrome supérieur — deux modes : barre horizontale classique ou barre latérale (style Arc).
 */
export const TopChrome: React.FC<TopChromeProps> = (props) => {
  const {
    tabs,
    activeTab,
    activeTabId,
    splitTabId,
    settings,
    omniboxText,
    omniboxFocused,
    omniboxInputRef,
    onOmniboxChange,
    onOmniboxSubmit,
    onOmniboxFocusChange,
    onSelectTab,
    onCloseTab,
    onNewTab,
    onToggleMute,
    onTogglePin,
    onSleepTab,
    onSplitWith,
    onBack,
    onForward,
    onReload,
    onHome,
    onStop,
    onToggleSplit,
    onShowFind,
    showFind,
    onZoom,
    onOpenPalette,
    onTogglePanel,
    activePanel,
    onOpenShield,
    onOpenReader,
    onOpenFocus,
    onToggleBookmark,
    isBookmarked,
    canOpenReader,
    shieldEnabled,
    shieldBlocked,
    downloadsActive,
    isMaximized,
    onMinimize,
    onMaximize,
    onCloseWindow,
    onUpdateSettings,
    onToggleTheme,
    bookmarks,
    history,
  } = props;

  const [engineOpen, setEngineOpen] = useState(false);
  const [moreOpen, setMoreOpen] = useState(false);
  const host = extractHostname(activeTab.url);

  const primaryActions = (
    <div className="app-no-drag flex items-center gap-1">
      <button
        type="button"
        onClick={onOpenShield}
        className={`chip !py-1.5 !px-2.5 cursor-pointer transition-all ${
          shieldEnabled
            ? '!bg-emerald-500/10 !border-emerald-500/25 !text-emerald-400 hover:!bg-emerald-500/18'
            : '!bg-amber-500/10 !border-amber-500/25 !text-amber-400'
        }`}
        title="Orakle Shield — protection & audit OWASP"
      >
        {shieldEnabled ? <ShieldCheck className="w-3.5 h-3.5" /> : <ShieldAlert className="w-3.5 h-3.5" />}
        <span className="tabular-nums text-[10.5px]">{shieldBlocked}</span>
      </button>

      <button
        type="button"
        onClick={onOpenReader}
        disabled={!canOpenReader}
        className="icon-btn w-8 h-8 hidden lg:inline-flex disabled:opacity-30"
        title="Mode Lecture (Ctrl+Shift+R)"
      >
        <BookOpen className="w-[15px] h-[15px]" />
      </button>

      <button
        type="button"
        onClick={onOpenFocus}
        className="icon-btn w-8 h-8 hidden xl:inline-flex"
        title="Mode Focus (Pomodoro)"
      >
        <Sparkles className="w-[15px] h-[15px]" />
      </button>

      <button
        type="button"
        onClick={onToggleSplit}
        className={`icon-btn w-8 h-8 ${splitTabId ? 'is-active' : ''}`}
        title="Écran scindé (Ctrl+\)"
      >
        <Columns2 className="w-[15px] h-[15px]" />
      </button>

      <button
        type="button"
        onClick={onOpenPalette}
        className="icon-btn w-8 h-8"
        title="Palette de commandes (Ctrl+K)"
      >
        <Command className="w-[15px] h-[15px]" />
      </button>

      <div className="w-px h-5 bg-[var(--stroke)] mx-0.5" />

      <div className="relative">
        <button
          type="button"
          onClick={() => setMoreOpen((v) => !v)}
          className={`icon-btn w-8 h-8 ${moreOpen ? 'is-active' : ''}`}
          title="Plus d’outils"
        >
          <MoreHorizontal className="w-[15px] h-[15px]" />
        </button>
        {moreOpen && (
          <>
            <div className="fixed inset-0 z-40" onClick={() => setMoreOpen(false)} />
            <div className="absolute right-0 top-[calc(100%+8px)] z-50 w-60 glass-panel-strong rounded-2xl p-1.5 animate-pop">
              {[
                { icon: <Bookmark className="w-3.5 h-3.5" />, label: 'Favoris', panel: 'bookmarks' as const },
                { icon: <Download className="w-3.5 h-3.5" />, label: 'Téléchargements', panel: 'downloads' as const },
                { icon: <FileText className="w-3.5 h-3.5" />, label: 'Notes', panel: 'notes' as const },
                { icon: <Layers className="w-3.5 h-3.5" />, label: 'Sessions de travail', panel: 'spaces' as const },
                { icon: <Cookie className="w-3.5 h-3.5" />, label: 'Cookies & traces', panel: 'privacy' as const },
                { icon: <Gauge className="w-3.5 h-3.5" />, label: 'Pulse — latence', panel: 'pulse' as const },
                { icon: <Settings className="w-3.5 h-3.5" />, label: 'Réglages', panel: 'settings' as const },
              ].map((item) => (
                <button
                  key={item.label}
                  type="button"
                  onClick={() => {
                    onTogglePanel(item.panel);
                    setMoreOpen(false);
                  }}
                  className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-[11.5px] font-medium transition-colors cursor-pointer text-left ${
                    activePanel === item.panel
                      ? 'bg-[var(--accent-soft)] text-[var(--accent)]'
                      : 'text-[var(--text-2)] hover:text-[var(--text)] hover:bg-[var(--fill-hover)]'
                  }`}
                >
                  {item.icon}
                  {item.label}
                </button>
              ))}
              <div className="my-1 h-px bg-[var(--stroke)]" />
              <div className="flex items-center gap-1 px-1.5 py-1">
                <span className="text-[10px] text-[var(--text-3)] mr-auto">Zoom</span>
                <button type="button" onClick={() => onZoom(-0.1)} className="icon-btn w-6 h-6">
                  <ZoomOut className="w-3 h-3" />
                </button>
                <span className="text-[10px] tabular-nums w-9 text-center text-[var(--text-2)]">
                  {Math.round(activeTab.zoomFactor * 100)}%
                </span>
                <button type="button" onClick={() => onZoom(0.1)} className="icon-btn w-6 h-6">
                  <ZoomIn className="w-3 h-3" />
                </button>
              </div>
              <button
                type="button"
                onClick={() => {
                  onToggleTheme();
                  setMoreOpen(false);
                }}
                className="w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-[11.5px] font-medium text-[var(--text-2)] hover:text-[var(--text)] hover:bg-[var(--fill-hover)] transition-colors cursor-pointer text-left"
              >
                {settings.theme === 'dark' ? (
                  <Sun className="w-3.5 h-3.5" />
                ) : (
                  <Moon className="w-3.5 h-3.5" />
                )}
                {settings.theme === 'dark' ? 'Thème Cristal' : 'Thème Abyssal'}
              </button>
              <button
                type="button"
                onClick={() => {
                  onUpdateSettings({
                    layoutMode: settings.layoutMode === 'horizontal' ? 'vertical' : 'horizontal',
                  });
                  setMoreOpen(false);
                }}
                className="w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-[11.5px] font-medium text-[var(--text-2)] hover:text-[var(--text)] hover:bg-[var(--fill-hover)] transition-colors cursor-pointer text-left"
              >
                <PanelRight className="w-3.5 h-3.5" />
                {settings.layoutMode === 'horizontal' ? 'Onglets latéraux' : 'Onglets en haut'}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );

  const leftNav = (
    <div className="app-no-drag flex items-center gap-0.5">
      <button type="button" onClick={onBack} disabled={!activeTab.canGoBack} className="icon-btn w-8 h-8" title="Précédent">
        <ArrowLeft className="w-[15px] h-[15px]" />
      </button>
      <button type="button" onClick={onForward} disabled={!activeTab.canGoForward} className="icon-btn w-8 h-8" title="Suivant">
        <ArrowRight className="w-[15px] h-[15px]" />
      </button>
      <button type="button" onClick={onReload} className="icon-btn w-8 h-8" title="Actualiser (Ctrl+R)">
        <RotateCw
          className={`w-[15px] h-[15px] ${activeTab.isLoading ? 'animate-spin text-[var(--accent)]' : ''}`}
        />
      </button>
      <button type="button" onClick={onHome} className="icon-btn w-8 h-8" title="Accueil">
        <Home className="w-[15px] h-[15px]" />
      </button>
    </div>
  );

  const engineSelector = (
    <div className="relative app-no-drag">
      <button
        type="button"
        onClick={() => setEngineOpen((v) => !v)}
        className="icon-btn h-8 px-2 gap-1 shrink-0"
        title="Moteur de recherche"
      >
        <span
          className="w-1.5 h-1.5 rounded-full"
          style={{ backgroundColor: SEARCH_ENGINES[settings.searchEngine].color }}
        />
        <span className="text-[10px] font-bold text-[var(--text-2)] hidden 2xl:inline">
          {SEARCH_ENGINES[settings.searchEngine].badge}
        </span>
        <ChevronDown className="w-2.5 h-2.5" />
      </button>
      {engineOpen && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setEngineOpen(false)} />
          <div className="absolute left-0 top-[calc(100%+8px)] z-50 w-56 glass-panel-strong rounded-2xl p-1.5 animate-pop">
            {(Object.keys(SEARCH_ENGINES) as SearchEngineId[]).map((id) => {
              const eng = SEARCH_ENGINES[id];
              return (
                <button
                  key={id}
                  type="button"
                  onClick={() => {
                    onUpdateSettings({ searchEngine: id });
                    setEngineOpen(false);
                  }}
                  className={`w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-left transition-colors cursor-pointer ${
                    settings.searchEngine === id
                      ? 'bg-[var(--accent-soft)]'
                      : 'hover:bg-[var(--fill-hover)]'
                  }`}
                >
                  <span className="w-1.5 h-1.5 rounded-full shrink-0" style={{ backgroundColor: eng.color }} />
                  <span className="text-[11.5px] font-semibold text-[var(--text)]">{eng.name}</span>
                  <span className="text-[9px] text-[var(--text-3)] ml-auto">{eng.hint}</span>
                </button>
              );
            })}
          </div>
        </>
      )}
    </div>
  );

  /* ================= MODE LATÉRAL (style Arc) ================= */
  if (settings.layoutMode === 'vertical') {
    return (
      <>
        <header className="app-drag glass-panel h-14 shrink-0 flex items-center gap-2 px-3 border-b border-[var(--stroke)] z-30">
          <div className="app-no-drag flex items-center gap-2 pr-1">
            <div
              className="w-7 h-7 rounded-xl flex items-center justify-center text-white font-black text-[11px] shadow-md shrink-0"
              style={{
                background:
                  'linear-gradient(135deg, var(--accent) 0%, color-mix(in srgb, var(--accent) 40%, var(--accent-2)) 100%)',
              }}
              title="Orakle Browser"
            >
              O
            </div>
          </div>
          {leftNav}
          <span className="chip app-no-drag !text-[10px] shrink-0 max-w-[180px] truncate" title={activeTab.url}>
            {host}
          </span>
          <Omnibox
            value={omniboxText}
            onChange={onOmniboxChange}
            onSubmit={onOmniboxSubmit}
            onOpenShield={onOpenShield}
            onOpenPalette={onOpenPalette}
            securityState={activeTab.securityState}
            zoomFactor={activeTab.zoomFactor}
            onResetZoom={() => onZoom(0)}
            searchEngine={settings.searchEngine}
            bookmarks={bookmarks}
            history={history}
            tabs={tabs}
            isBookmarked={isBookmarked}
            onToggleBookmark={onToggleBookmark}
            shieldBlocked={shieldBlocked}
            shieldEnabled={shieldEnabled}
            focused={omniboxFocused}
            onFocusChange={onOmniboxFocusChange}
            inputRef={omniboxInputRef}
          />
          {primaryActions}
          <WindowControls
            isMaximized={isMaximized}
            onMinimize={onMinimize}
            onMaximize={onMaximize}
            onClose={onCloseWindow}
          />
        </header>
      </>
    );
  }

  /* ================= MODE HORIZONTAL (classique modernisé) ================= */
  return (
    <header className="z-30 shrink-0">
      <div className="glass-panel border-b border-[var(--stroke)]">
        {/* Ligne 1 : onglets */}
        {!settings.compactTabsBar && (
          <div className="app-drag flex items-center gap-2 px-2.5 pt-2 pb-1">
            <div className="app-no-drag flex items-center gap-2 pr-1 shrink-0">
              <div
                className="w-7 h-7 rounded-xl flex items-center justify-center text-white font-black text-[11px] shadow-md"
                style={{
                  background:
                    'linear-gradient(135deg, var(--accent) 0%, color-mix(in srgb, var(--accent) 40%, var(--accent-2)) 100%)',
                  boxShadow: '0 6px 18px -8px var(--accent-glow), inset 0 1px 0 rgba(255,255,255,0.4)',
                }}
                title="Orakle Browser — Aurora"
              >
                O
              </div>
            </div>

            <TabStrip
              tabs={tabs}
              activeTabId={activeTabId}
              splitTabId={splitTabId}
              onSelect={onSelectTab}
              onClose={onCloseTab}
              onNewTab={onNewTab}
              onToggleMute={onToggleMute}
              onTogglePin={onTogglePin}
              onSleepTab={onSleepTab}
              onSplitWith={onSplitWith}
            />

            <WindowControls
              isMaximized={isMaximized}
              onMinimize={onMinimize}
              onMaximize={onMaximize}
              onClose={onCloseWindow}
            />
          </div>
        )}

        {/* Ligne 2 : navigation + omnibox + actions */}
        <div className="app-drag flex items-center gap-2 px-2.5 py-2">
          {settings.compactTabsBar && (
            <div className="app-no-drag flex items-center gap-2 pr-1 shrink-0">
              <div
                className="w-7 h-7 rounded-xl flex items-center justify-center text-white font-black text-[11px]"
                style={{ background: 'linear-gradient(135deg, var(--accent), var(--accent-2))' }}
              >
                O
              </div>
            </div>
          )}
          {leftNav}
          {engineSelector}
          <Omnibox
            value={omniboxText}
            onChange={onOmniboxChange}
            onSubmit={onOmniboxSubmit}
            onOpenShield={onOpenShield}
            onOpenPalette={onOpenPalette}
            securityState={activeTab.securityState}
            zoomFactor={activeTab.zoomFactor}
            onResetZoom={() => onZoom(0)}
            searchEngine={settings.searchEngine}
            bookmarks={bookmarks}
            history={history}
            tabs={tabs}
            isBookmarked={isBookmarked}
            onToggleBookmark={onToggleBookmark}
            shieldBlocked={shieldBlocked}
            shieldEnabled={shieldEnabled}
            focused={omniboxFocused}
            onFocusChange={onOmniboxFocusChange}
            inputRef={omniboxInputRef}
          />
          {primaryActions}
          {settings.compactTabsBar && (
            <WindowControls
              isMaximized={isMaximized}
              onMinimize={onMinimize}
              onMaximize={onMaximize}
              onClose={onCloseWindow}
            />
          )}
        </div>

        {/* Ligne 3 : favoris */}
        {settings.showBookmarksBar && (
          <div className="flex items-center gap-1 px-3 pb-1.5 overflow-x-auto no-scrollbar">
            <Star className="w-3 h-3 text-[var(--text-3)] shrink-0" />
            {bookmarks.length === 0 ? (
              <button
                type="button"
                onClick={() => onTogglePanel('bookmarks')}
                className="text-[10.5px] font-medium text-[var(--text-3)] hover:text-[var(--accent)] transition-colors cursor-pointer px-1"
              >
                Aucun favori — ajoutez-en depuis la barre d’adresse
              </button>
            ) : (
              bookmarks.map((bm) => (
                <button
                  key={bm.id}
                  type="button"
                  onClick={() => onOmniboxSubmit(bm.url)}
                  className="px-2 py-0.5 rounded-md text-[10.5px] font-medium text-[var(--text-2)] hover:text-[var(--text)] hover:bg-[var(--fill-hover)] flex items-center gap-1.5 shrink-0 transition-colors cursor-pointer"
                  title={bm.url}
                >
                  <span
                    className="w-1 h-1 rounded-full shrink-0"
                    style={{ backgroundColor: 'var(--accent)' }}
                  />
                  <span className="truncate max-w-[130px]">{bm.title}</span>
                </button>
              ))
            )}
          </div>
        )}
      </div>

      {/* Barre de progression */}
      {activeTab.isLoading && (
        <div className="w-full h-[2px] bg-transparent overflow-hidden">
          <div
            className="w-1/2 h-full loading-bar-indeterminate"
            style={{ background: 'linear-gradient(90deg, transparent, var(--accent), var(--accent-2), transparent)' }}
          />
        </div>
      )}
    </header>
  );
};
