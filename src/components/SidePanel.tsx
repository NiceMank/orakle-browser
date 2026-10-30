import React, { useState } from 'react';
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
} from 'lucide-react';
import {
  SidePanelTab,
  BookmarkItem,
  HistoryItem,
  DownloadItemInfo,
  BrowserSettings,
  AccentColor,
  SearchEngineId,
} from '../types/browser';
import { SEARCH_ENGINES, formatBytes } from '../utils/url';

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
}

const ACCENT_SWATCHES: { id: AccentColor; label: string; color: string }[] = [
  { id: 'orakle', label: 'Bleu Orakle', color: '#0a84ff' },
  { id: 'cyber', label: 'Violet Cyber', color: '#8b5cf6' },
  { id: 'emerald', label: 'Émeraude', color: '#10b981' },
  { id: 'amber', label: 'Ambre Solaire', color: '#f59e0b' },
  { id: 'rose', label: 'Rose Néon', color: '#ec4899' },
];

export const SidePanel: React.FC<SidePanelProps> = ({
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
}) => {
  const [searchFilter, setSearchFilter] = useState('');
  const [newBmTitle, setNewBmTitle] = useState('');
  const [newBmUrl, setNewBmUrl] = useState('');
  const [newBmCategory, setNewBmCategory] = useState('Général');
  const [showAddBm, setShowAddBm] = useState(false);
  const [clearedFeedback, setClearedFeedback] = useState(false);

  if (!activeTab) return null;

  const filteredBookmarks = bookmarks.filter(
    (b) =>
      b.title.toLowerCase().includes(searchFilter.toLowerCase()) ||
      b.url.toLowerCase().includes(searchFilter.toLowerCase()) ||
      b.category.toLowerCase().includes(searchFilter.toLowerCase())
  );

  const filteredHistory = history.filter(
    (h) =>
      h.title.toLowerCase().includes(searchFilter.toLowerCase()) ||
      h.url.toLowerCase().includes(searchFilter.toLowerCase())
  );

  const handleCreateBookmark = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newBmTitle.trim() || !newBmUrl.trim()) return;
    const url = /^https?:\/\//i.test(newBmUrl.trim())
      ? newBmUrl.trim()
      : `https://${newBmUrl.trim()}`;
    onAddBookmark({
      title: newBmTitle.trim(),
      url,
      category: newBmCategory.trim() || 'Général',
    });
    setNewBmTitle('');
    setNewBmUrl('');
    setShowAddBm(false);
  };

  const handlePurgeAll = async () => {
    await onClearBrowsingData();
    setClearedFeedback(true);
    setTimeout(() => setClearedFeedback(false), 2500);
  };

  return (
    <aside className="glass-panel-strong w-96 h-full border-l border-[var(--stroke)] flex flex-col shrink-0 z-30 select-none">
      {/* Barre d'icônes du panneau latéral */}
      <div className="p-3 border-b border-[var(--stroke)] flex items-center justify-between">
        <div className="flex items-center gap-1">
          {[
            { id: 'bookmarks', icon: Bookmark, label: 'Favoris' },
            { id: 'history', icon: Clock, label: 'Historique' },
            { id: 'downloads', icon: Download, label: 'Téléchargements' },
            { id: 'notes', icon: FileText, label: 'Scratchpad' },
            { id: 'settings', icon: Settings, label: 'Paramètres' },
          ].map((item) => {
            const Icon = item.icon;
            const active = activeTab === item.id;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => onSelectPanel(item.id as SidePanelTab)}
                className={`px-2.5 py-1.5 rounded-xl text-xs font-semibold flex items-center gap-1.5 transition-all cursor-pointer ${
                  active
                    ? 'bg-[var(--accent-soft)] text-[var(--accent)]'
                    : 'text-[var(--text-2)] hover:text-[var(--text)] hover:bg-[var(--fill)]'
                }`}
                title={item.label}
              >
                <Icon className="w-4 h-4" />
                {active && <span>{item.label}</span>}
              </button>
            );
          })}
        </div>

        <button
          type="button"
          onClick={onClose}
          className="icon-btn w-7 h-7"
          aria-label="Fermer le panneau latéral"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Contenu dynamique selon la section */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {(activeTab === 'bookmarks' || activeTab === 'history') && (
          <div className="flex items-center gap-2 px-3 py-2 rounded-xl bg-[var(--fill)] border border-[var(--stroke)]">
            <Search className="w-4 h-4 text-[var(--text-3)]" />
            <input
              type="text"
              value={searchFilter}
              onChange={(e) => setSearchFilter(e.target.value)}
              placeholder={
                activeTab === 'bookmarks'
                  ? 'Rechercher dans vos favoris...'
                  : "Rechercher dans l'historique..."
              }
              className="w-full bg-transparent text-xs text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none"
            />
          </div>
        )}

        {/* Section Favoris */}
        {activeTab === 'bookmarks' && (
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-bold text-[var(--text-2)]">
                {filteredBookmarks.length} favoris enregistrés
              </span>
              <button
                type="button"
                onClick={() => setShowAddBm(!showAddBm)}
                className="text-xs font-semibold text-[var(--accent)] flex items-center gap-1 hover:underline cursor-pointer"
              >
                <Plus className="w-3.5 h-3.5" />
                Nouveau
              </button>
            </div>

            {showAddBm && (
              <form onSubmit={handleCreateBookmark} className="orakle-card p-3 space-y-2">
                <input
                  type="text"
                  placeholder="Titre du favori"
                  value={newBmTitle}
                  onChange={(e) => setNewBmTitle(e.target.value)}
                  className="w-full px-2.5 py-1.5 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] text-xs text-[var(--text)]"
                  required
                />
                <input
                  type="text"
                  placeholder="https://..."
                  value={newBmUrl}
                  onChange={(e) => setNewBmUrl(e.target.value)}
                  className="w-full px-2.5 py-1.5 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] text-xs text-[var(--text)]"
                  required
                />
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    placeholder="Catégorie"
                    value={newBmCategory}
                    onChange={(e) => setNewBmCategory(e.target.value)}
                    className="flex-1 px-2.5 py-1.5 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] text-xs text-[var(--text)]"
                  />
                  <button
                    type="submit"
                    className="px-3 py-1.5 rounded-lg text-xs font-bold text-white cursor-pointer"
                    style={{ backgroundColor: 'var(--accent)' }}
                  >
                    Ajouter
                  </button>
                </div>
              </form>
            )}

            <div className="space-y-2">
              {filteredBookmarks.map((bm) => (
                <div
                  key={bm.id}
                  onClick={() => onNavigate(bm.url)}
                  className="orakle-card p-3 flex items-center justify-between gap-2 cursor-pointer group"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-xs font-semibold text-[var(--text)] truncate group-hover:text-[var(--accent)]">
                        {bm.title}
                      </span>
                      <span className="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-[var(--fill-strong)] text-[var(--text-2)]">
                        {bm.category}
                      </span>
                    </div>
                    <div className="text-[10px] text-[var(--text-3)] truncate mt-0.5">
                      {bm.url}
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      onRemoveBookmark(bm.id);
                    }}
                    className="icon-btn w-7 h-7 opacity-0 group-hover:opacity-100 hover:text-red-400"
                    title="Supprimer ce favori"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Section Historique */}
        {activeTab === 'history' && (
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-bold text-[var(--text-2)]">
                {filteredHistory.length} visites
              </span>
              {history.length > 0 && (
                <button
                  type="button"
                  onClick={onClearHistory}
                  className="text-xs font-semibold text-red-400 hover:underline flex items-center gap-1 cursor-pointer"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                  Tout effacer
                </button>
              )}
            </div>

            {filteredHistory.length === 0 ? (
              <div className="orakle-card p-8 text-center text-xs text-[var(--text-3)]">
                Aucun historique correspondant.
              </div>
            ) : (
              <div className="space-y-1.5">
                {filteredHistory.map((item) => (
                  <div
                    key={item.id}
                    onClick={() => onNavigate(item.url)}
                    className="orakle-card p-2.5 flex items-center justify-between gap-2 cursor-pointer group"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="text-xs font-medium text-[var(--text)] truncate group-hover:text-[var(--accent)]">
                        {item.title}
                      </div>
                      <div className="text-[10px] text-[var(--text-3)] truncate">{item.url}</div>
                    </div>
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        onRemoveHistoryItem(item.id);
                      }}
                      className="icon-btn w-6 h-6 opacity-0 group-hover:opacity-100"
                    >
                      <X className="w-3 h-3" />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Section Téléchargements */}
        {activeTab === 'downloads' && (
          <div className="space-y-3">
            <div className="text-xs font-bold text-[var(--text-2)]">
              Gestionnaire de téléchargements ({downloads.length})
            </div>
            {downloads.length === 0 ? (
              <div className="orakle-card p-8 text-center text-xs text-[var(--text-3)]">
                Aucun téléchargement récent.
              </div>
            ) : (
              downloads.map((dl) => {
                const percent =
                  dl.totalBytes > 0
                    ? Math.min(100, Math.round((dl.receivedBytes / dl.totalBytes) * 100))
                    : 100;
                return (
                  <div key={dl.id} className="orakle-card p-3 space-y-2">
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-xs font-semibold text-[var(--text)] truncate">
                        {dl.filename}
                      </span>
                      <span className="text-[10px] font-bold uppercase text-[var(--accent)]">
                        {dl.state === 'completed' ? 'Terminé' : `${percent}%`}
                      </span>
                    </div>
                    {dl.state === 'progressing' && (
                      <div className="w-full h-1.5 rounded-full bg-[var(--fill-strong)] overflow-hidden">
                        <div
                          className="h-full rounded-full transition-all"
                          style={{
                            width: `${percent}%`,
                            backgroundColor: 'var(--accent)',
                          }}
                        />
                      </div>
                    )}
                    <div className="flex items-center justify-between text-[10px] text-[var(--text-3)]">
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
                            <ExternalLink className="w-3 h-3" />
                            Ouvrir
                          </button>
                          <button
                            type="button"
                            onClick={() => window.orakle?.downloads.showInFolder(dl.savePath)}
                            className="text-[var(--text-2)] hover:text-[var(--text)] flex items-center gap-1 cursor-pointer"
                          >
                            <FolderOpen className="w-3 h-3" />
                            Dossier
                          </button>
                        </div>
                      )}
                    </div>
                  </div>
                );
              })
            )}
          </div>
        )}

        {/* Section Scratchpad / Notes */}
        {activeTab === 'notes' && (
          <div className="h-full flex flex-col space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-xs font-bold text-[var(--text-2)]">
                Bloc-notes persistant Orakle
              </span>
              <span className="text-[10px] text-[var(--text-3)]">Synchronisé en temps réel</span>
            </div>
            <textarea
              value={scratchpad}
              onChange={(e) => onUpdateScratchpad(e.target.value)}
              placeholder="Écrivez vos notes, fragments de code ou listes de tâches..."
              className="w-full flex-1 min-h-[380px] resize-none rounded-2xl bg-[var(--fill)] border border-[var(--stroke)] p-3.5 text-xs font-mono text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
            />
          </div>
        )}

        {/* Section Paramètres */}
        {activeTab === 'settings' && (
          <div className="space-y-4">
            {/* Apparence Clair / Sombre */}
            <div className="orakle-card p-3.5 space-y-2.5">
              <div className="text-xs font-bold text-[var(--text)] flex items-center gap-2">
                {settings.theme === 'dark' ? (
                  <Moon className="w-4 h-4 text-[var(--accent)]" />
                ) : (
                  <Sun className="w-4 h-4 text-amber-400" />
                )}
                Thème Visuel
              </div>
              <div className="grid grid-cols-2 gap-2">
                <button
                  type="button"
                  onClick={() => onUpdateSettings({ theme: 'dark' })}
                  className={`py-2 px-3 rounded-xl text-xs font-semibold border cursor-pointer flex items-center justify-center gap-2 ${
                    settings.theme === 'dark'
                      ? 'bg-[var(--accent-soft)] border-[var(--accent)] text-[var(--text)]'
                      : 'bg-[var(--fill)] border-[var(--stroke)] text-[var(--text-2)]'
                  }`}
                >
                  <Moon className="w-3.5 h-3.5" />
                  Sombre Abyssal
                </button>
                <button
                  type="button"
                  onClick={() => onUpdateSettings({ theme: 'light' })}
                  className={`py-2 px-3 rounded-xl text-xs font-semibold border cursor-pointer flex items-center justify-center gap-2 ${
                    settings.theme === 'light'
                      ? 'bg-[var(--accent-soft)] border-[var(--accent)] text-[var(--text)]'
                      : 'bg-[var(--fill)] border-[var(--stroke)] text-[var(--text-2)]'
                  }`}
                >
                  <Sun className="w-3.5 h-3.5" />
                  Clair Cristal
                </button>
              </div>
            </div>

            {/* Couleur d'accentuation */}
            <div className="orakle-card p-3.5 space-y-2.5">
              <div className="text-xs font-bold text-[var(--text)] flex items-center gap-2">
                <Palette className="w-4 h-4 text-[var(--accent)]" />
                Couleur d’Accentuation
              </div>
              <div className="flex items-center justify-between gap-2">
                {ACCENT_SWATCHES.map((sw) => (
                  <button
                    key={sw.id}
                    type="button"
                    onClick={() => onUpdateSettings({ accent: sw.id })}
                    className={`w-10 h-10 rounded-2xl flex items-center justify-center transition-transform cursor-pointer border-2 ${
                      settings.accent === sw.id
                        ? 'scale-110 border-white shadow-lg'
                        : 'border-transparent opacity-75 hover:opacity-100'
                    }`}
                    style={{ backgroundColor: sw.color }}
                    title={sw.label}
                  />
                ))}
              </div>
            </div>

            {/* Disposition des onglets */}
            <div className="orakle-card p-3.5 space-y-2.5">
              <div className="text-xs font-bold text-[var(--text)] flex items-center gap-2">
                <Layout className="w-4 h-4 text-[var(--accent)]" />
                Disposition des Onglets
              </div>
              <div className="grid grid-cols-2 gap-2">
                <button
                  type="button"
                  onClick={() => onUpdateSettings({ layoutMode: 'horizontal' })}
                  className={`py-2 px-3 rounded-xl text-xs font-semibold border cursor-pointer ${
                    settings.layoutMode === 'horizontal'
                      ? 'bg-[var(--accent-soft)] border-[var(--accent)] text-[var(--text)]'
                      : 'bg-[var(--fill)] border-[var(--stroke)] text-[var(--text-2)]'
                  }`}
                >
                  Classique (Haut)
                </button>
                <button
                  type="button"
                  onClick={() => onUpdateSettings({ layoutMode: 'vertical' })}
                  className={`py-2 px-3 rounded-xl text-xs font-semibold border cursor-pointer ${
                    settings.layoutMode === 'vertical'
                      ? 'bg-[var(--accent-soft)] border-[var(--accent)] text-[var(--text)]'
                      : 'bg-[var(--fill)] border-[var(--stroke)] text-[var(--text-2)]'
                  }`}
                >
                  Verticale (Style Arc)
                </button>
              </div>

              <label className="flex items-center justify-between pt-2 text-xs text-[var(--text-2)] cursor-pointer">
                <span>Afficher la barre de favoris</span>
                <input
                  type="checkbox"
                  checked={settings.showBookmarksBar}
                  onChange={(e) => onUpdateSettings({ showBookmarksBar: e.target.checked })}
                  className="w-4 h-4 accent-[var(--accent)]"
                />
              </label>
            </div>

            {/* Moteur de recherche par défaut */}
            <div className="orakle-card p-3.5 space-y-2">
              <div className="text-xs font-bold text-[var(--text)]">
                Moteur de Recherche par Défaut
              </div>
              <select
                value={settings.searchEngine}
                onChange={(e) =>
                  onUpdateSettings({ searchEngine: e.target.value as SearchEngineId })
                }
                className="w-full px-3 py-2 rounded-xl bg-[var(--fill)] border border-[var(--stroke)] text-xs text-[var(--text)]"
              >
                {(Object.keys(SEARCH_ENGINES) as SearchEngineId[]).map((id) => (
                  <option key={id} value={id} className="bg-[#141620] text-white">
                    {SEARCH_ENGINES[id].name}
                  </option>
                ))}
              </select>
            </div>

            {/* Nettoyage complet des traces */}
            <div className="orakle-card p-3.5 space-y-2.5 border-red-500/20">
              <div className="text-xs font-bold text-red-400 flex items-center gap-2">
                <ShieldAlert className="w-4 h-4" />
                Confidentialité & Purge Sanitaire
              </div>
              <p className="text-[11px] text-[var(--text-2)]">
                Efface instantanément le cache, les cookies, le stockage local et l’historique de navigation.
              </p>
              <button
                type="button"
                onClick={handlePurgeAll}
                className="w-full py-2 rounded-xl text-xs font-bold bg-red-500/15 hover:bg-red-500/25 text-red-400 border border-red-500/30 transition-colors cursor-pointer"
              >
                {clearedFeedback
                  ? '✓ Toutes les traces ont été purgées !'
                  : 'Purger les données de navigation'}
              </button>
            </div>
          </div>
        )}
      </div>
    </aside>
  );
};
