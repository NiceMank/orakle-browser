import React, { useMemo, useState } from 'react';
import {
  Bookmark,
  Clock,
  Download,
  FileText,
  Settings,
  X,
  Trash2,
  ExternalLink,
  FolderOpen,
  Search,
  Sun,
  Moon,
  Layout,
  Palette,
  ShieldAlert,
  Plus,
  Cookie,
  Gauge,
  Layers,
  Sparkles,
  MonitorSmartphone,
  Eye,
  Wand2,
  Brain,
  Check,
  ArrowRight,
} from 'lucide-react';
import {
  SidePanelTab,
  BookmarkItem,
  HistoryItem,
  DownloadItemInfo,
  BrowserSettings,
  AccentColor,
  SearchEngineId,
  SavedSession,
  Space,
  Tab,
} from '../types/browser';
import { SEARCH_ENGINES, formatBytes, groupByDay, prettyUrl, relativeTime, extractHostname } from '../utils/url';
import { PrivacyPanel } from './PrivacyPanel';
import { PulsePanel } from './PulsePanel';
import { SessionsPanel } from './SessionsPanel';

interface SidePanelProps {
  activeTab: SidePanelTab;
  onClose: () => void;
  onSelectPanel: (tab: SidePanelTab) => void;
  onNavigate: (url: string) => void;
  bookmarks: BookmarkItem[];
  onAddBookmark: (bm: { title: string; url: string; category: string }) => void;
  onRemoveBookmark: (id: string) => void;
  history: HistoryItem[];
  onClearHistory: () => void;
  onRemoveHistoryItem: (id: string) => void;
  downloads: DownloadItemInfo[];
  scratchpad: string;
  onUpdateScratchpad: (content: string) => void;
  settings: BrowserSettings;
  onUpdateSettings: (patch: Partial<BrowserSettings>) => void;
  onClearBrowsingData: () => void;
  currentUrl: string;
  tabs: Tab[];
  sessions: SavedSession[];
  onSaveSession: (session: { name: string; emoji: string; tabs: { title: string; url: string }[] }) => void;
  onDeleteSession: (id: string) => void;
  onRestoreSession: (session: SavedSession) => void;
  onOpenUrls: (urls: string[]) => void;
  spaces: Space[];
  onSaveSpaces: (spaces: Space[]) => void;
  onOpenReader: () => void;
}

const ACCENT_SWATCHES: { id: AccentColor; label: string; color: string }[] = [
  { id: 'orakle', label: 'Bleu Orakle', color: '#0a84ff' },
  { id: 'cyber', label: 'Violet Cyber', color: '#8b5cf6' },
  { id: 'emerald', label: 'Émeraude', color: '#10b981' },
  { id: 'amber', label: 'Ambre Solaire', color: '#f59e0b' },
  { id: 'rose', label: 'Rose Néon', color: '#ec4899' },
  { id: 'ice', label: 'Glace Arctique', color: '#38bdf8' },
  { id: 'graphite', label: 'Graphite', color: '#94a3b8' },
];

const PANELS: { id: Exclude<SidePanelTab, null>; icon: React.ElementType; label: string }[] = [
  { id: 'bookmarks', icon: Bookmark, label: 'Favoris' },
  { id: 'history', icon: Clock, label: 'Historique' },
  { id: 'downloads', icon: Download, label: 'Téléchargements' },
  { id: 'notes', icon: FileText, label: 'Notes' },
  { id: 'privacy', icon: Cookie, label: 'Confidentialité' },
  { id: 'pulse', icon: Gauge, label: 'Pulse' },
  { id: 'spaces', icon: Layers, label: 'Sessions' },
  { id: 'settings', icon: Settings, label: 'Réglages' },
];

export const SidePanel: React.FC<SidePanelProps> = (props) => {
  const {
    activeTab,
    onClose,
    onSelectPanel,
    onNavigate,
    bookmarks,
    onAddBookmark,
    onRemoveBookmark,
    history,
    onClearHistory,
    onRemoveHistoryItem,
    downloads,
    scratchpad,
    onUpdateScratchpad,
    settings,
    onUpdateSettings,
    onClearBrowsingData,
    currentUrl,
    tabs,
    sessions,
    onSaveSession,
    onDeleteSession,
    onRestoreSession,
    onOpenUrls,
    onOpenReader,
  } = props;

  const [searchFilter, setSearchFilter] = useState('');
  const [newBmTitle, setNewBmTitle] = useState('');
  const [newBmUrl, setNewBmUrl] = useState('');
  const [newBmCategory, setNewBmCategory] = useState('Général');
  const [showAddBm, setShowAddBm] = useState(false);
  const [clearedFeedback, setClearedFeedback] = useState(false);
  const [focusDomain, setFocusDomain] = useState('');

  const currentHost = useMemo(() => extractHostname(currentUrl), [currentUrl]);

  if (!activeTab) return null;

  const handlePurgeAll = async () => {
    onClearBrowsingData();
    setClearedFeedback(true);
    setTimeout(() => setClearedFeedback(false), 2600);
  };

  const filteredBookmarks = bookmarks.filter(
    (b) =>
      !searchFilter ||
      b.title.toLowerCase().includes(searchFilter.toLowerCase()) ||
      b.url.toLowerCase().includes(searchFilter.toLowerCase())
  );

  const filteredHistory = history.filter(
    (h) =>
      !searchFilter ||
      h.title.toLowerCase().includes(searchFilter.toLowerCase()) ||
      h.url.toLowerCase().includes(searchFilter.toLowerCase())
  );

  const groupedHistory = groupByDay(filteredHistory);

  return (
    <aside className="w-[340px] h-full border-l border-[var(--stroke)] flex flex-col shrink-0 z-20 animate-fade-up" style={{ background: 'var(--bg-glass-strong)', boxShadow: 'inset 0 1px 0 var(--stroke-specular)' }}>
      {/* En-tête avec sélecteur de panneaux */}
      <div className="px-3 pt-3 pb-2 border-b border-[var(--stroke)] space-y-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            {PANELS.map((p) => {
              const Icon = p.icon;
              const active = activeTab === p.id;
              return (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => {
                    setSearchFilter('');
                    onSelectPanel(active ? null : p.id);
                  }}
                  className={`icon-btn w-7 h-7 !rounded-lg ${active ? 'is-active' : ''}`}
                  title={p.label}
                  aria-label={p.label}
                >
                  <Icon className="w-3.5 h-3.5" />
                </button>
              );
            })}
          </div>
          <button type="button" onClick={onClose} className="icon-btn w-7 h-7" aria-label="Fermer le panneau">
            <X className="w-3.5 h-3.5" />
          </button>
        </div>

        {(activeTab === 'bookmarks' || activeTab === 'history') && (
          <div className="flex items-center gap-2 px-2.5 py-1.5 rounded-xl bg-[var(--fill)] border border-[var(--stroke)]">
            <Search className="w-3.5 h-3.5 text-[var(--text-3)] shrink-0" />
            <input
              value={searchFilter}
              onChange={(e) => setSearchFilter(e.target.value)}
              placeholder="Filtrer…"
              className="w-full bg-transparent text-[11.5px] text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none"
            />
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto px-3 py-3">
        {/* ---------------- FAVORIS ---------------- */}
        {activeTab === 'bookmarks' && (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-[10px] font-bold uppercase tracking-[0.14em] text-[var(--text-3)]">
                {filteredBookmarks.length} favori(s)
              </span>
              <button
                type="button"
                onClick={() => setShowAddBm((v) => !v)}
                className="text-[11px] font-semibold text-[var(--accent)] hover:opacity-80 flex items-center gap-1 cursor-pointer"
              >
                <Plus className="w-3 h-3" />
                Nouveau
              </button>
            </div>

            {showAddBm && (
              <div className="orakle-card p-2.5 space-y-1.5">
                <input
                  value={newBmTitle}
                  onChange={(e) => setNewBmTitle(e.target.value)}
                  placeholder="Titre"
                  className="w-full px-2 py-1.5 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] text-[11.5px] text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
                />
                <input
                  value={newBmUrl}
                  onChange={(e) => setNewBmUrl(e.target.value)}
                  placeholder="https://…"
                  className="w-full px-2 py-1.5 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] text-[11.5px] text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
                />
                <div className="flex gap-1.5">
                  <input
                    value={newBmCategory}
                    onChange={(e) => setNewBmCategory(e.target.value)}
                    placeholder="Catégorie"
                    className="flex-1 px-2 py-1.5 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] text-[11.5px] text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
                  />
                  <button
                    type="button"
                    onClick={() => {
                      if (!newBmTitle.trim() || !newBmUrl.trim()) return;
                      onAddBookmark({
                        title: newBmTitle.trim(),
                        url: /^https?:\/\//i.test(newBmUrl.trim())
                          ? newBmUrl.trim()
                          : `https://${newBmUrl.trim()}`,
                        category: newBmCategory || 'Général',
                      });
                      setNewBmTitle('');
                      setNewBmUrl('');
                      setShowAddBm(false);
                    }}
                    className="btn-primary !py-1.5 !px-3 !text-[11px]"
                  >
                    <Check className="w-3 h-3" />
                  </button>
                </div>
              </div>
            )}

            {filteredBookmarks.map((bm) => (
              <div
                key={bm.id}
                className="group flex items-center gap-2 px-2.5 py-2 rounded-xl hover:bg-[var(--fill)] border border-transparent hover:border-[var(--stroke)] transition-all"
              >
                <div
                  className="w-6 h-6 rounded-lg flex items-center justify-center text-[9px] font-extrabold text-white shrink-0"
                  style={{ backgroundColor: 'var(--accent)' }}
                >
                  {bm.title.slice(0, 2).toUpperCase()}
                </div>
                <button
                  type="button"
                  onClick={() => onNavigate(bm.url)}
                  className="min-w-0 flex-1 text-left cursor-pointer"
                >
                  <div className="text-[11.5px] font-semibold text-[var(--text)] truncate">{bm.title}</div>
                  <div className="text-[9.5px] text-[var(--text-3)] truncate">{prettyUrl(bm.url, 42)}</div>
                </button>
                <span className="chip !text-[8.5px] shrink-0 hidden group-hover:inline-flex">{bm.category}</span>
                <button
                  type="button"
                  onClick={() => onRemoveBookmark(bm.id)}
                  className="icon-btn w-6 h-6 opacity-0 group-hover:opacity-100 hover:text-red-400"
                  title="Supprimer"
                >
                  <Trash2 className="w-3 h-3" />
                </button>
              </div>
            ))}
          </div>
        )}

        {/* ---------------- HISTORIQUE ---------------- */}
        {activeTab === 'history' && (
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-[10px] font-bold uppercase tracking-[0.14em] text-[var(--text-3)]">
                {filteredHistory.length} visite(s)
              </span>
              <button
                type="button"
                onClick={onClearHistory}
                className="text-[11px] font-semibold text-red-400 hover:opacity-80 flex items-center gap-1 cursor-pointer"
              >
                <Trash2 className="w-3 h-3" />
                Effacer
              </button>
            </div>

            {groupedHistory.length === 0 && (
              <div className="orakle-card p-6 text-center text-[11px] text-[var(--text-3)]">
                Aucun historique pour l’instant.
              </div>
            )}

            {groupedHistory.map(([day, items]) => (
              <div key={day} className="space-y-1">
                <div className="text-[9.5px] font-bold uppercase tracking-[0.12em] text-[var(--text-3)] capitalize px-1">
                  {day}
                </div>
                {items.map((item) => (
                  <div
                    key={item.id}
                    className="group flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-[var(--fill)] transition-colors"
                  >
                    <span className="w-1 h-1 rounded-full bg-[var(--text-3)] shrink-0" />
                    <button
                      type="button"
                      onClick={() => onNavigate(item.url)}
                      className="min-w-0 flex-1 text-left cursor-pointer"
                    >
                      <div className="text-[11px] font-medium text-[var(--text)] truncate group-hover:text-[var(--accent)]">
                        {item.title}
                      </div>
                      <div className="text-[9px] text-[var(--text-3)] truncate">
                        {prettyUrl(item.url, 40)} · {relativeTime(item.visitedAt)}
                      </div>
                    </button>
                    <button
                      type="button"
                      onClick={() => onRemoveHistoryItem(item.id)}
                      className="icon-btn w-6 h-6 opacity-0 group-hover:opacity-100 hover:text-red-400"
                    >
                      <Trash2 className="w-2.5 h-2.5" />
                    </button>
                  </div>
                ))}
              </div>
            ))}
          </div>
        )}

        {/* ---------------- TÉLÉCHARGEMENTS ---------------- */}
        {activeTab === 'downloads' && (
          <div className="space-y-2">
            <div className="text-[10px] font-bold uppercase tracking-[0.14em] text-[var(--text-3)]">
              {downloads.length} fichier(s)
            </div>
            {downloads.length === 0 && (
              <div className="orakle-card p-6 text-center text-[11px] text-[var(--text-3)]">
                Aucun téléchargement.
              </div>
            )}
            {downloads.map((dl) => {
              const percent =
                dl.totalBytes > 0
                  ? Math.min(100, Math.round((dl.receivedBytes / dl.totalBytes) * 100))
                  : dl.state === 'completed'
                  ? 100
                  : 0;
              return (
                <div key={dl.id} className="orakle-card !rounded-xl p-2.5 space-y-1.5">
                  <div className="flex items-center gap-2">
                    <span className="text-[11px] font-semibold text-[var(--text)] truncate flex-1">
                      {dl.filename}
                    </span>
                    <span
                      className={`text-[9.5px] font-bold uppercase ${
                        dl.state === 'completed' ? 'text-emerald-400' : 'text-[var(--accent)]'
                      }`}
                    >
                      {dl.state === 'completed' ? 'Terminé' : dl.state === 'progressing' ? `${percent}%` : dl.state}
                    </span>
                  </div>
                  {dl.state === 'progressing' && (
                    <div className="w-full h-1 rounded-full bg-[var(--fill-strong)] overflow-hidden">
                      <div
                        className="h-full rounded-full transition-all"
                        style={{ width: `${percent}%`, backgroundColor: 'var(--accent)' }}
                      />
                    </div>
                  )}
                  <div className="flex items-center justify-between text-[9.5px] text-[var(--text-3)]">
                    <span>
                      {formatBytes(dl.receivedBytes)}
                      {dl.totalBytes > 0 ? ` / ${formatBytes(dl.totalBytes)}` : ''}
                    </span>
                    {dl.savePath && (
                      <div className="flex items-center gap-2">
                        <button
                          type="button"
                          onClick={() => window.orakle?.downloads.openFile(dl.savePath)}
                          className="text-[var(--accent)] hover:underline flex items-center gap-1 cursor-pointer"
                        >
                          <ExternalLink className="w-2.5 h-2.5" />
                          Ouvrir
                        </button>
                        <button
                          type="button"
                          onClick={() => window.orakle?.downloads.showInFolder(dl.savePath)}
                          className="text-[var(--text-2)] hover:text-[var(--text)] flex items-center gap-1 cursor-pointer"
                        >
                          <FolderOpen className="w-2.5 h-2.5" />
                          Dossier
                        </button>
                      </div>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {/* ---------------- NOTES ---------------- */}
        {activeTab === 'notes' && (
          <div className="h-full flex flex-col space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-[10px] font-bold uppercase tracking-[0.14em] text-[var(--text-3)]">
                Scratchpad persistant
              </span>
              <span className="text-[9.5px] text-[var(--text-3)]">{scratchpad.length} car.</span>
            </div>
            <textarea
              value={scratchpad}
              onChange={(e) => onUpdateScratchpad(e.target.value)}
              placeholder="Notes, fragments de code, listes de tâches…"
              className="w-full flex-1 min-h-[420px] resize-none rounded-2xl bg-[var(--fill)] border border-[var(--stroke)] p-3 text-[11.5px] font-mono leading-relaxed text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
            />
          </div>
        )}

        {/* ---------------- CONFIDENTIALITÉ ---------------- */}
        {activeTab === 'privacy' && <PrivacyPanel currentHost={currentHost} />}

        {/* ---------------- PULSE ---------------- */}
        {activeTab === 'pulse' && <PulsePanel currentUrl={currentUrl} />}

        {/* ---------------- SESSIONS ---------------- */}
        {activeTab === 'spaces' && (
          <SessionsPanel
            tabs={tabs}
            sessions={sessions}
            onSaveSession={onSaveSession}
            onDeleteSession={onDeleteSession}
            onRestoreSession={onRestoreSession}
            onOpenUrls={onOpenUrls}
          />
        )}

        {/* ---------------- RÉGLAGES ---------------- */}
        {activeTab === 'settings' && (
          <div className="space-y-2.5">
            <Section title="Thème" icon={settings.theme === 'dark' ? Moon : Sun}>
              <div className="segmented w-full justify-center">
                <button type="button" data-active={settings.theme === 'dark'} onClick={() => onUpdateSettings({ theme: 'dark' })}>
                  Abyssal
                </button>
                <button type="button" data-active={settings.theme === 'light'} onClick={() => onUpdateSettings({ theme: 'light' })}>
                  Cristal
                </button>
              </div>
            </Section>

            <Section title="Accentuation" icon={Palette}>
              <div className="flex items-center justify-between gap-1">
                {ACCENT_SWATCHES.map((sw) => (
                  <button
                    key={sw.id}
                    type="button"
                    onClick={() => onUpdateSettings({ accent: sw.id })}
                    className={`w-8 h-8 rounded-xl flex items-center justify-center transition-transform cursor-pointer border-2 ${
                      settings.accent === sw.id ? 'scale-110 border-white shadow-lg' : 'border-transparent opacity-70 hover:opacity-100'
                    }`}
                    style={{ backgroundColor: sw.color }}
                    title={sw.label}
                  />
                ))}
              </div>
            </Section>

            <Section title="Verre & effet" icon={Sparkles}>
              <div className="space-y-2">
                <div className="segmented w-full justify-center">
                  {(
                    [
                      ['thin', 'Léger'],
                      ['regular', 'Standard'],
                      ['thick', 'Épais'],
                    ] as const
                  ).map(([id, label]) => (
                    <button
                      key={id}
                      type="button"
                      data-active={settings.glassDensity === id}
                      onClick={() => onUpdateSettings({ glassDensity: id })}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                <Toggle
                  label="Nappe Aurora animée"
                  checked={settings.showAurora}
                  onChange={(v) => onUpdateSettings({ showAurora: v })}
                />
                <Toggle
                  label="Grain filmique"
                  checked={settings.showNoise}
                  onChange={(v) => onUpdateSettings({ showNoise: v })}
                />
                <Toggle
                  label="Grille technique"
                  checked={settings.showGrid}
                  onChange={(v) => onUpdateSettings({ showGrid: v })}
                />
                <Toggle
                  label="Mouvement réduit"
                  checked={settings.motionMode === 'calm'}
                  onChange={(v) => onUpdateSettings({ motionMode: v ? 'calm' : 'full' })}
                />
                <Toggle
                  label="Réduire les transparences (perf)"
                  checked={settings.reduceTransparency}
                  onChange={(v) => onUpdateSettings({ reduceTransparency: v })}
                />
              </div>
            </Section>

            <Section title="Disposition" icon={Layout}>
              <div className="grid grid-cols-2 gap-1.5">
                <button
                  type="button"
                  onClick={() => onUpdateSettings({ layoutMode: 'horizontal' })}
                  className={`py-2 rounded-xl text-[11px] font-semibold border cursor-pointer flex items-center justify-center gap-1.5 ${
                    settings.layoutMode === 'horizontal'
                      ? 'bg-[var(--accent-soft)] border-[var(--accent)] text-[var(--text)]'
                      : 'bg-[var(--fill)] border-[var(--stroke)] text-[var(--text-2)]'
                  }`}
                >
                  <MonitorSmartphone className="w-3 h-3" />
                  Haut
                </button>
                <button
                  type="button"
                  onClick={() => onUpdateSettings({ layoutMode: 'vertical' })}
                  className={`py-2 rounded-xl text-[11px] font-semibold border cursor-pointer flex items-center justify-center gap-1.5 ${
                    settings.layoutMode === 'vertical'
                      ? 'bg-[var(--accent-soft)] border-[var(--accent)] text-[var(--text)]'
                      : 'bg-[var(--fill)] border-[var(--stroke)] text-[var(--text-2)]'
                  }`}
                >
                  <Layout className="w-3 h-3" />
                  Latéral
                </button>
              </div>
              <Toggle
                label="Barre de favoris"
                checked={settings.showBookmarksBar}
                onChange={(v) => onUpdateSettings({ showBookmarksBar: v })}
              />
            </Section>

            <Section title="Mode Lecture" icon={Eye}>
              <div className="flex items-center gap-2">
                <span className="text-[10.5px] text-[var(--text-2)] shrink-0">Taille</span>
                <input
                  type="range"
                  min={13}
                  max={26}
                  value={settings.readerFontSize}
                  onChange={(e) => onUpdateSettings({ readerFontSize: Number(e.target.value) })}
                  className="orakle-range flex-1"
                  style={{ ['--range-progress' as string]: `${((settings.readerFontSize - 13) / 13) * 100}%` }}
                />
                <span className="text-[10.5px] font-bold tabular-nums w-6 text-right">{settings.readerFontSize}</span>
              </div>
              <button type="button" onClick={onOpenReader} className="btn-glass w-full !justify-center !py-1.5">
                <ArrowRight className="w-3 h-3" />
                Ouvrir le lecteur sur la page actuelle
              </button>
            </Section>

            <Section title="Assistant Orakle" icon={Brain}>
              <div className="space-y-2">
                <select
                  value={settings.aiProvider}
                  onChange={(e) => onUpdateSettings({ aiProvider: e.target.value as BrowserSettings['aiProvider'] })}
                  className="w-full px-2.5 py-2 rounded-xl bg-[var(--fill)] border border-[var(--stroke)] text-[11px] text-[var(--text)] focus:outline-none"
                >
                  <option value="pollinations">Pollinations (gratuit, sans clé)</option>
                  <option value="openai-compatible">API compatible OpenAI (clé requise)</option>
                  <option value="offline">100 % local (extraction de phrases)</option>
                </select>
                {settings.aiProvider === 'openai-compatible' && (
                  <>
                    <input
                      value={settings.aiEndpoint}
                      onChange={(e) => onUpdateSettings({ aiEndpoint: e.target.value })}
                      placeholder="https://api.openai.com/v1/chat/completions"
                      className="w-full px-2.5 py-1.5 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] text-[10.5px] text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
                    />
                    <input
                      value={settings.aiModel}
                      onChange={(e) => onUpdateSettings({ aiModel: e.target.value })}
                      placeholder="gpt-4o-mini"
                      className="w-full px-2.5 py-1.5 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] text-[10.5px] text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
                    />
                    <p className="text-[9.5px] text-[var(--text-3)] leading-relaxed">
                      La clé API n’est jamais stockée dans l’interface : exportez
                      <code className="mx-1 px-1 rounded bg-[var(--fill-strong)]">ORAKLE_AI_KEY</code>
                      avant de lancer l’application.
                    </p>
                  </>
                )}
                <Toggle
                  label="Résumé automatique à l’ouverture du lecteur"
                  checked={settings.aiAutoSummarize}
                  onChange={(v) => onUpdateSettings({ aiAutoSummarize: v })}
                />
              </div>
            </Section>

            <Section title="Focus — sites à bloquer" icon={Wand2}>
              <div className="flex gap-1.5">
                <input
                  value={focusDomain}
                  onChange={(e) => setFocusDomain(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && focusDomain.trim()) {
                      onUpdateSettings({
                        focusBlockList: Array.from(
                          new Set([...settings.focusBlockList, focusDomain.trim().toLowerCase()])
                        ),
                      });
                      setFocusDomain('');
                    }
                  }}
                  placeholder="ex: youtube.com"
                  className="flex-1 px-2 py-1.5 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] text-[10.5px] text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
                />
                <button
                  type="button"
                  onClick={() => {
                    if (!focusDomain.trim()) return;
                    onUpdateSettings({
                      focusBlockList: Array.from(
                        new Set([...settings.focusBlockList, focusDomain.trim().toLowerCase()])
                      ),
                    });
                    setFocusDomain('');
                  }}
                  className="btn-glass !py-1 !px-2"
                >
                  <Plus className="w-3 h-3" />
                </button>
              </div>
              <div className="flex flex-wrap gap-1 pt-1">
                {settings.focusBlockList.map((d) => (
                  <button
                    key={d}
                    type="button"
                    onClick={() =>
                      onUpdateSettings({ focusBlockList: settings.focusBlockList.filter((x) => x !== d) })
                    }
                    className="chip !text-[9px] hover:!text-red-400 transition-colors cursor-pointer"
                    title="Retirer de la liste"
                  >
                    {d}
                    <X className="w-2.5 h-2.5" />
                  </button>
                ))}
              </div>
            </Section>

            <Section title="Moteur de recherche" icon={Search}>
              <select
                value={settings.searchEngine}
                onChange={(e) => onUpdateSettings({ searchEngine: e.target.value as SearchEngineId })}
                className="w-full px-2.5 py-2 rounded-xl bg-[var(--fill)] border border-[var(--stroke)] text-[11px] text-[var(--text)] focus:outline-none"
              >
                {(Object.keys(SEARCH_ENGINES) as SearchEngineId[]).map((id) => (
                  <option key={id} value={id}>
                    {SEARCH_ENGINES[id].name} — {SEARCH_ENGINES[id].hint}
                  </option>
                ))}
              </select>
            </Section>

            <div className="orakle-card p-3 space-y-2 border-red-500/20">
              <div className="text-[11px] font-bold text-red-400 flex items-center gap-2">
                <ShieldAlert className="w-3.5 h-3.5" />
                Purge sanitaire
              </div>
              <p className="text-[10px] leading-relaxed text-[var(--text-2)]">
                Efface cache, cookies, stockage local et historique de navigation.
              </p>
              <button
                type="button"
                onClick={handlePurgeAll}
                className="w-full py-2 rounded-xl text-[11px] font-bold bg-red-500/12 hover:bg-red-500/22 text-red-400 border border-red-500/28 transition-colors cursor-pointer"
              >
                {clearedFeedback ? '✓ Traces purgées' : 'Purger les données'}
              </button>
            </div>

            <div className="text-center text-[9.5px] text-[var(--text-3)] pt-1 pb-2">
              Orakle Browser v1.1 « Aurora » · Electron + React 18
            </div>
          </div>
        )}
      </div>
    </aside>
  );
};

/* ---------- Sous-composants ---------- */

const Section: React.FC<{ title: string; icon: React.ElementType; children: React.ReactNode }> = ({
  title,
  icon: Icon,
  children,
}) => (
  <div className="orakle-card p-3 space-y-2">
    <div className="text-[11px] font-bold text-[var(--text)] flex items-center gap-1.5">
      <Icon className="w-3.5 h-3.5 text-[var(--accent)]" />
      {title}
    </div>
    {children}
  </div>
);

const Toggle: React.FC<{ label: string; checked: boolean; onChange: (v: boolean) => void }> = ({
  label,
  checked,
  onChange,
}) => (
  <label className="flex items-center justify-between gap-2 text-[10.5px] text-[var(--text-2)] cursor-pointer select-none">
    <span>{label}</span>
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className="switch"
      data-on={checked}
    />
  </label>
);
