import React, { useState, useEffect } from 'react';
import {
  Search,
  Shield,
  ShieldCheck,
  Sparkles,
  Plus,
  Trash2,
  ExternalLink,
  Clock,
  FileText,
  Zap,
  Globe,
  Lock,
  ArrowRight,
  Compass,
} from 'lucide-react';
import {
  SpeedDialItem,
  ShieldStats,
  HistoryItem,
  SearchEngineId,
} from '../types/browser';
import { SEARCH_ENGINES, resolveOmniboxInput, formatBytes, extractHostname } from '../utils/url';

interface NewTabPageProps {
  onNavigate: (url: string) => void;
  searchEngine: SearchEngineId;
  onChangeSearchEngine: (engine: SearchEngineId) => void;
  speedDials: SpeedDialItem[];
  onUpdateSpeedDials: (dials: SpeedDialItem[]) => void;
  shieldStats: ShieldStats;
  onOpenShieldModal: () => void;
  recentHistory: HistoryItem[];
  scratchpad: string;
  onUpdateScratchpad: (val: string) => void;
  isPrivate: boolean;
}

export const NewTabPage: React.FC<NewTabPageProps> = ({
  onNavigate,
  searchEngine,
  onChangeSearchEngine,
  speedDials,
  onUpdateSpeedDials,
  shieldStats,
  onOpenShieldModal,
  recentHistory,
  scratchpad,
  onUpdateScratchpad,
  isPrivate,
}) => {
  const [query, setQuery] = useState('');
  const [now, setNow] = useState(new Date());
  const [showAddDial, setShowAddDial] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [newUrl, setNewUrl] = useState('');
  const [newColor, setNewColor] = useState('#0a84ff');

  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 10000);
    return () => clearInterval(timer);
  }, []);

  const handleSearchSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!query.trim()) return;
    onNavigate(resolveOmniboxInput(query, searchEngine));
  };

  const handleAddDial = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newTitle.trim() || !newUrl.trim()) return;
    const formattedUrl = /^https?:\/\//i.test(newUrl.trim())
      ? newUrl.trim()
      : `https://${newUrl.trim()}`;
    const cleanTitle = newTitle.trim();
    const badge = cleanTitle.slice(0, 3).toUpperCase();
    const item: SpeedDialItem = {
      id: `sd-${Date.now()}`,
      title: cleanTitle,
      url: formattedUrl,
      color: newColor,
      badge,
    };
    onUpdateSpeedDials([...speedDials, item]);
    setNewTitle('');
    setNewUrl('');
    setShowAddDial(false);
  };

  const handleRemoveDial = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    onUpdateSpeedDials(speedDials.filter((d) => d.id !== id));
  };

  const timeString = now.toLocaleTimeString('fr-FR', {
    hour: '2-digit',
    minute: '2-digit',
  });

  const dateString = now.toLocaleDateString('fr-FR', {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
  });

  return (
    <div className="relative w-full h-full overflow-y-auto select-none px-6 py-10 md:px-12">
      {/* Halo ambiant décoratif */}
      <div
        className="pointer-events-none fixed inset-x-0 top-0 h-96 opacity-35 blur-3xl transition-all duration-700"
        style={{
          background: isPrivate
            ? 'radial-gradient(circle at 50% 0%, #8b5cf6 0%, transparent 70%)'
            : 'radial-gradient(circle at 50% 0%, var(--accent) 0%, transparent 70%)',
        }}
      />

      <div className="relative z-10 mx-auto max-w-5xl space-y-8">
        {/* En-tête Horloge & Statut */}
        <header className="flex flex-col items-center text-center space-y-2 pt-2">
          {isPrivate && (
            <div className="inline-flex items-center gap-2 px-3.5 py-1 rounded-full text-xs font-semibold tracking-wide uppercase bg-purple-500/20 text-purple-300 border border-purple-500/30 mb-2">
              <Lock className="w-3.5 h-3.5" />
              Session Fantôme (Navigation Privée Isolée)
            </div>
          )}

          <div className="text-6xl md:text-7xl font-extrabold tracking-tight tabular-nums bg-clip-text text-transparent bg-gradient-to-b from-[var(--text)] to-[var(--text-2)]">
            {timeString}
          </div>

          <p className="text-sm font-medium capitalize text-[var(--text-2)] tracking-wide">
            {dateString} · <span className="text-[var(--accent)] font-semibold">Orakle Browser</span>
          </p>
        </header>

        {/* Barre de recherche centrale multi-moteurs */}
        <section className="mx-auto max-w-2xl space-y-3">
          <form
            onSubmit={handleSearchSubmit}
            className="orakle-card flex items-center gap-3 px-4 py-3.5 focus-within:ring-2 focus-within:ring-[var(--accent)] transition-all"
          >
            <Search className="w-5 h-5 text-[var(--accent)] shrink-0" />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={`Rechercher sur ${SEARCH_ENGINES[searchEngine].name} ou saisir une adresse (!yt, !gh, !w)...`}
              className="w-full bg-transparent text-base text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none"
              autoFocus
            />
            {query.trim() && (
              <button
                type="submit"
                className="px-3 py-1.5 rounded-xl text-xs font-semibold text-white flex items-center gap-1.5 transition-transform active:scale-95 cursor-pointer"
                style={{ backgroundColor: 'var(--accent)' }}
              >
                Aller
                <ArrowRight className="w-3.5 h-3.5" />
              </button>
            )}
          </form>

          {/* Sélecteur de moteur de recherche rapide */}
          <div className="flex flex-wrap items-center justify-center gap-2">
            {(Object.keys(SEARCH_ENGINES) as SearchEngineId[]).map((id) => {
              const eng = SEARCH_ENGINES[id];
              const active = searchEngine === id;
              return (
                <button
                  key={id}
                  type="button"
                  onClick={() => onChangeSearchEngine(id)}
                  className={`px-3 py-1 rounded-full text-xs font-medium flex items-center gap-1.5 transition-all cursor-pointer border ${
                    active
                      ? 'bg-[var(--accent-soft)] text-[var(--text)] border-[var(--accent)] shadow-sm'
                      : 'bg-[var(--fill)] text-[var(--text-2)] border-[var(--stroke)] hover:text-[var(--text)]'
                  }`}
                >
                  <span
                    className="w-2 h-2 rounded-full"
                    style={{ backgroundColor: eng.color }}
                  />
                  {eng.name}
                </button>
              );
            })}
          </div>
        </section>

        {/* Bandeau Bento : Orakle Shield (Télémétrie Cyber en direct) */}
        <section className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3.5">
          <button
            type="button"
            onClick={onOpenShieldModal}
            className="orakle-card p-4 text-left flex items-center gap-3.5 cursor-pointer group"
          >
            <div className="w-11 h-11 rounded-2xl flex items-center justify-center bg-emerald-500/15 text-emerald-400 border border-emerald-500/25 shrink-0 group-hover:scale-105 transition-transform">
              <ShieldCheck className="w-6 h-6" />
            </div>
            <div>
              <div className="text-2xl font-extrabold tabular-nums text-[var(--text)]">
                {shieldStats.totalBlocked}
              </div>
              <div className="text-xs text-[var(--text-2)] font-medium">
                Menaces & Traqueurs bloqués
              </div>
            </div>
          </button>

          <button
            type="button"
            onClick={onOpenShieldModal}
            className="orakle-card p-4 text-left flex items-center gap-3.5 cursor-pointer group"
          >
            <div className="w-11 h-11 rounded-2xl flex items-center justify-center bg-blue-500/15 text-blue-400 border border-blue-500/25 shrink-0 group-hover:scale-105 transition-transform">
              <Sparkles className="w-6 h-6" />
            </div>
            <div>
              <div className="text-2xl font-extrabold tabular-nums text-[var(--text)]">
                {shieldStats.paramsStripped}
              </div>
              <div className="text-xs text-[var(--text-2)] font-medium">
                Liens nettoyés (UTM / Pistage)
              </div>
            </div>
          </button>

          <button
            type="button"
            onClick={onOpenShieldModal}
            className="orakle-card p-4 text-left flex items-center gap-3.5 cursor-pointer group"
          >
            <div className="w-11 h-11 rounded-2xl flex items-center justify-center bg-amber-500/15 text-amber-400 border border-amber-500/25 shrink-0 group-hover:scale-105 transition-transform">
              <Zap className="w-6 h-6" />
            </div>
            <div>
              <div className="text-2xl font-extrabold tabular-nums text-[var(--text)]">
                {formatBytes(shieldStats.estimatedBytesSaved)}
              </div>
              <div className="text-xs text-[var(--text-2)] font-medium">
                Données économisées
              </div>
            </div>
          </button>

          <button
            type="button"
            onClick={onOpenShieldModal}
            className="orakle-card p-4 text-left flex items-center gap-3.5 cursor-pointer group"
          >
            <div className="w-11 h-11 rounded-2xl flex items-center justify-center bg-purple-500/15 text-purple-400 border border-purple-500/25 shrink-0 group-hover:scale-105 transition-transform">
              <Shield className="w-6 h-6" />
            </div>
            <div>
              <div className="text-sm font-bold text-[var(--text)] flex items-center gap-1.5">
                {shieldStats.enabled ? 'Bouclier Actif' : 'Bouclier En Pause'}
                <span
                  className={`w-2 h-2 rounded-full ${
                    shieldStats.enabled ? 'bg-emerald-400' : 'bg-amber-400'
                  }`}
                />
              </div>
              <div className="text-xs text-[var(--text-2)] font-medium mt-0.5">
                DNT + GPC · Profil {shieldStats.userAgentProfile}
              </div>
            </div>
          </button>
        </section>

        {/* Accès Rapides (Speed Dials) */}
        <section className="space-y-3">
          <div className="flex items-center justify-between">
            <h2 className="text-xs font-bold uppercase tracking-wider text-[var(--text-2)] flex items-center gap-1.5">
              <Compass className="w-4 h-4 text-[var(--accent)]" />
              Accès Rapides
            </h2>
            <button
              type="button"
              onClick={() => setShowAddDial(!showAddDial)}
              className="text-xs font-semibold text-[var(--accent)] hover:underline flex items-center gap-1 cursor-pointer"
            >
              <Plus className="w-3.5 h-3.5" />
              Ajouter un raccourci
            </button>
          </div>

          {showAddDial && (
            <form
              onSubmit={handleAddDial}
              className="orakle-card p-4 flex flex-wrap items-center gap-3"
            >
              <input
                type="text"
                placeholder="Nom du site (ex: GitLab)"
                value={newTitle}
                onChange={(e) => setNewTitle(e.target.value)}
                className="px-3 py-2 rounded-xl bg-[var(--fill)] border border-[var(--stroke)] text-sm text-[var(--text)] focus:outline-none"
                required
              />
              <input
                type="text"
                placeholder="URL (ex: https://gitlab.com)"
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
                className="flex-1 min-w-[200px] px-3 py-2 rounded-xl bg-[var(--fill)] border border-[var(--stroke)] text-sm text-[var(--text)] focus:outline-none"
                required
              />
              <input
                type="color"
                value={newColor}
                onChange={(e) => setNewColor(e.target.value)}
                className="w-9 h-9 rounded-lg border border-[var(--stroke)] cursor-pointer bg-transparent"
                title="Couleur du badge"
              />
              <button
                type="submit"
                className="px-4 py-2 rounded-xl text-xs font-bold text-white cursor-pointer"
                style={{ backgroundColor: 'var(--accent)' }}
              >
                Enregistrer
              </button>
            </form>
          )}

          <div className="grid grid-cols-2 sm:grid-cols-4 md:grid-cols-8 gap-3">
            {speedDials.map((dial) => (
              <div
                key={dial.id}
                onClick={() => onNavigate(dial.url)}
                className="orakle-card group relative p-3.5 flex flex-col items-center justify-center text-center gap-2.5 cursor-pointer hover:-translate-y-0.5"
              >
                <button
                  type="button"
                  onClick={(e) => handleRemoveDial(e, dial.id)}
                  className="opacity-0 group-hover:opacity-100 absolute top-1.5 right-1.5 p-1 rounded-lg bg-black/40 text-gray-300 hover:text-red-400 transition-opacity cursor-pointer"
                  title="Supprimer ce raccourci"
                  aria-label={`Supprimer ${dial.title}`}
                >
                  <Trash2 className="w-3 h-3" />
                </button>

                <div
                  className="w-11 h-11 rounded-2xl flex items-center justify-center text-xs font-extrabold text-white shadow-md transition-transform group-hover:scale-105"
                  style={{
                    backgroundColor: dial.color,
                    boxShadow: `0 6px 16px -4px ${dial.color}66`,
                  }}
                >
                  {dial.badge}
                </div>

                <div className="w-full">
                  <div className="text-xs font-semibold text-[var(--text)] truncate">
                    {dial.title}
                  </div>
                  <div className="text-[10px] text-[var(--text-3)] truncate">
                    {extractHostname(dial.url)}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </section>

        {/* Section Inférieure : Scratchpad Rapide & Activité Récente */}
        <section className="grid grid-cols-1 md:grid-cols-2 gap-4 pb-6">
          {/* Bloc-notes rapide persistant */}
          <div className="orakle-card p-4 flex flex-col space-y-2.5">
            <div className="flex items-center justify-between">
              <h3 className="text-xs font-bold uppercase tracking-wider text-[var(--text-2)] flex items-center gap-1.5">
                <FileText className="w-4 h-4 text-[var(--accent)]" />
                Scratchpad & Notes Rapides
              </h3>
              <span className="text-[11px] text-[var(--text-3)]">Sauvegarde auto</span>
            </div>
            <textarea
              value={scratchpad}
              onChange={(e) => onUpdateScratchpad(e.target.value)}
              placeholder="Notez vos idées, commandes, extraits ou URLs à analyser..."
              rows={5}
              className="w-full flex-1 resize-none rounded-xl bg-[var(--fill)] border border-[var(--stroke)] p-3 text-xs font-mono text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none focus:border-[var(--accent)]"
            />
          </div>

          {/* Historique récent */}
          <div className="orakle-card p-4 flex flex-col space-y-2.5">
            <div className="flex items-center justify-between">
              <h3 className="text-xs font-bold uppercase tracking-wider text-[var(--text-2)] flex items-center gap-1.5">
                <Clock className="w-4 h-4 text-[var(--accent)]" />
                Navigation Récente
              </h3>
              <span className="text-[11px] text-[var(--text-3)]">
                {recentHistory.length} entrées
              </span>
            </div>

            {recentHistory.length === 0 ? (
              <div className="flex-1 flex flex-col items-center justify-center text-center py-8 text-[var(--text-3)] text-xs">
                <Globe className="w-7 h-7 mb-2 opacity-40" />
                Aucune page visitée récemment.
              </div>
            ) : (
              <div className="space-y-1.5 max-h-40 overflow-y-auto pr-1">
                {recentHistory.slice(0, 6).map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => onNavigate(item.url)}
                    className="w-full flex items-center justify-between gap-2 px-3 py-2 rounded-xl hover:bg-[var(--fill-hover)] text-left transition-colors cursor-pointer group"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="text-xs font-medium text-[var(--text)] truncate group-hover:text-[var(--accent)]">
                        {item.title}
                      </div>
                      <div className="text-[10px] text-[var(--text-3)] truncate">
                        {item.url}
                      </div>
                    </div>
                    <ExternalLink className="w-3.5 h-3.5 text-[var(--text-3)] opacity-0 group-hover:opacity-100 shrink-0" />
                  </button>
                ))}
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
};
