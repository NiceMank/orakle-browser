import React, { useEffect, useMemo, useState } from 'react';
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
  BookOpen,
  Layers,
  Play,
  ArrowUpRight,
  ChevronRight,
} from 'lucide-react';
import {
  SpeedDialItem,
  ShieldStats,
  HistoryItem,
  SearchEngineId,
  SavedSession,
  Space,
} from '../types/browser';
import { SEARCH_ENGINES, resolveOmniboxInput, formatBytes, extractHostname, prettyUrl } from '../utils/url';

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
  spaces: Space[];
  activeSpaceId: string;
  sessions: SavedSession[];
  onRestoreSession: (session: SavedSession) => void;
  onOpenFocus: () => void;
  tabCount: number;
}

const PALETTE = ['#0a84ff', '#8b5cf6', '#10b981', '#f59e0b', '#ec4899', '#38bdf8', '#f43f5e'];

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
  spaces,
  activeSpaceId,
  sessions,
  onRestoreSession,
  onOpenFocus,
  tabCount,
}) => {
  const [query, setQuery] = useState('');
  const [now, setNow] = useState(new Date());
  const [showAddDial, setShowAddDial] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [newUrl, setNewUrl] = useState('');

  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 15000);
    return () => clearInterval(timer);
  }, []);

  const space = spaces.find((s) => s.id === activeSpaceId);

  const greeting = useMemo(() => {
    const h = now.getHours();
    if (h < 6) return 'Bonne nuit';
    if (h < 12) return 'Bonjour';
    if (h < 18) return 'Bon après-midi';
    return 'Bonsoir';
  }, [now]);

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
    onUpdateSpeedDials([
      ...speedDials,
      {
        id: `sd-${Date.now()}`,
        title: cleanTitle,
        url: formattedUrl,
        color: PALETTE[speedDials.length % PALETTE.length],
        badge: cleanTitle.slice(0, 3).toUpperCase(),
      },
    ]);
    setNewTitle('');
    setNewUrl('');
    setShowAddDial(false);
  };

  const timeString = now.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' });
  const dateString = now.toLocaleDateString('fr-FR', {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
  });

  return (
    <div className="relative w-full h-full overflow-y-auto px-5 py-8 md:px-10 md:py-12">
      <div className="relative z-10 mx-auto max-w-[1080px] space-y-7">
        {/* ---------- En-tête éditorial ---------- */}
        <header className="space-y-4">
          <div className="flex items-center gap-2.5 flex-wrap">
            {isPrivate && (
              <span className="chip !bg-purple-500/15 !border-purple-500/30 !text-[var(--c-cyber)]">
                <Lock className="w-3 h-3" />
                Session Fantôme
              </span>
            )}
            {space && (
              <span
                className="chip"
                style={{
                  color: space.color,
                  borderColor: `color-mix(in srgb, ${space.color} 32%, transparent)`,
                  background: `color-mix(in srgb, ${space.color} 10%, transparent)`,
                }}
              >
                <span>{space.emoji}</span>
                {space.name}
              </span>
            )}
            <span className="chip">
              <Layers className="w-3 h-3" />
              {tabCount} onglet{tabCount > 1 ? 's' : ''}
            </span>
            <button type="button" onClick={onOpenFocus} className="chip hover:border-[var(--accent)] transition-colors cursor-pointer">
              <Sparkles className="w-3 h-3 text-[var(--accent)]" />
              Mode Focus
            </button>
          </div>

          <div className="flex items-end justify-between gap-6 flex-wrap">
            <div>
              <p className="text-[13px] font-medium text-[var(--text-2)] tracking-tight">
                {greeting} — <span className="capitalize">{dateString}</span>
              </p>
              <h1 className="text-[clamp(3rem,7vw,5rem)] font-extrabold leading-[0.95] tracking-[-0.045em] tabular-nums mt-1">
                <span className="shimmer-text">{timeString}</span>
              </h1>
            </div>
            <div className="hidden md:flex items-center gap-2 pb-2">
              <div className="text-right">
                <div className="text-[10px] uppercase tracking-[0.16em] text-[var(--text-3)]">
                  Orakle Aurora
                </div>
                <div className="text-[11px] font-semibold text-[var(--text-2)]">
                  v1.1 · Glass edition
                </div>
              </div>
            </div>
          </div>
        </header>

        {/* ---------- Barre de recherche ---------- */}
        <section className="space-y-3">
          <form
            onSubmit={handleSearchSubmit}
            className="group flex items-center gap-3 px-4 py-3.5 rounded-[22px] bg-[var(--bg-glass-strong)] border border-[var(--stroke)] shadow-[0_18px_50px_-30px_rgba(0,0,0,0.9),inset_0_1px_0_var(--stroke-specular)] focus-within:border-[color-mix(in_srgb,var(--accent)_55%,transparent)] focus-within:shadow-[0_0_0_4px_var(--accent-soft),0_20px_50px_-28px_var(--accent-glow)] transition-all"
          >
            <Search className="w-5 h-5 text-[var(--accent)] shrink-0" />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={`Rechercher, saisir une URL ou un !bang…`}
              className="w-full bg-transparent text-[15px] text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none focus-visible:outline-none tracking-[-0.01em]"
              autoFocus
            />
            <span className="hidden sm:flex items-center gap-1 text-[10px] text-[var(--text-3)]">
              <kbd className="kbd">Ctrl</kbd>
              <kbd className="kbd">L</kbd>
            </span>
            <button
              type="submit"
              className="btn-primary !px-4 !py-2 shrink-0"
              aria-label="Rechercher"
            >
              Aller
              <ArrowRight className="w-3.5 h-3.5" />
            </button>
          </form>

          <div className="flex flex-wrap items-center gap-1.5">
            {(Object.keys(SEARCH_ENGINES) as SearchEngineId[]).map((id) => {
              const eng = SEARCH_ENGINES[id];
              const active = searchEngine === id;
              return (
                <button
                  key={id}
                  type="button"
                  onClick={() => onChangeSearchEngine(id)}
                  className={`px-2.5 py-1 rounded-full text-[11px] font-semibold flex items-center gap-1.5 transition-all cursor-pointer border ${
                    active
                      ? 'bg-[var(--accent-soft)] text-[var(--text)] border-[color-mix(in_srgb,var(--accent)_45%,transparent)]'
                      : 'bg-[var(--fill)] text-[var(--text-2)] border-[var(--stroke)] hover:text-[var(--text)] hover:border-[var(--stroke-strong)]'
                  }`}
                  title={eng.hint}
                >
                  <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: eng.color }} />
                  {eng.name}
                </button>
              );
            })}
          </div>
        </section>

        {/* ---------- Bandeau télémétrie ---------- */}
        <section className="grid grid-cols-2 lg:grid-cols-4 gap-2.5">
          <button type="button" onClick={onOpenShieldModal} className="orakle-card p-3.5 text-left flex items-center gap-3 cursor-pointer">
            <div className="w-10 h-10 rounded-2xl flex items-center justify-center bg-emerald-500/12 text-emerald-400 border border-emerald-500/22 shrink-0">
              <ShieldCheck className="w-5 h-5" />
            </div>
            <div className="min-w-0">
              <div className="text-xl font-extrabold tabular-nums leading-none">{shieldStats.totalBlocked}</div>
              <div className="text-[10.5px] text-[var(--text-2)] font-medium mt-1 truncate">Menaces bloquées</div>
            </div>
          </button>

          <button type="button" onClick={onOpenShieldModal} className="orakle-card p-3.5 text-left flex items-center gap-3 cursor-pointer">
            <div className="w-10 h-10 rounded-2xl flex items-center justify-center bg-sky-500/12 text-sky-400 border border-sky-500/22 shrink-0">
              <Sparkles className="w-5 h-5" />
            </div>
            <div className="min-w-0">
              <div className="text-xl font-extrabold tabular-nums leading-none">{shieldStats.paramsStripped}</div>
              <div className="text-[10.5px] text-[var(--text-2)] font-medium mt-1 truncate">Liens nettoyés</div>
            </div>
          </button>

          <button type="button" onClick={onOpenShieldModal} className="orakle-card p-3.5 text-left flex items-center gap-3 cursor-pointer">
            <div className="w-10 h-10 rounded-2xl flex items-center justify-center bg-amber-500/12 text-amber-400 border border-amber-500/22 shrink-0">
              <Zap className="w-5 h-5" />
            </div>
            <div className="min-w-0">
              <div className="text-xl font-extrabold tabular-nums leading-none">
                {formatBytes(shieldStats.estimatedBytesSaved)}
              </div>
              <div className="text-[10.5px] text-[var(--text-2)] font-medium mt-1 truncate">Données économisées</div>
            </div>
          </button>

          <button type="button" onClick={onOpenShieldModal} className="orakle-card p-3.5 text-left flex items-center gap-3 cursor-pointer">
            <div
              className={`w-10 h-10 rounded-2xl flex items-center justify-center shrink-0 border ${
                shieldStats.enabled
                  ? 'bg-emerald-500/12 text-emerald-400 border-emerald-500/22'
                  : 'bg-amber-500/12 text-amber-400 border-amber-500/22'
              }`}
            >
              <Shield className="w-5 h-5" />
            </div>
            <div className="min-w-0">
              <div className="text-[12px] font-bold flex items-center gap-1.5">
                {shieldStats.enabled ? 'Bouclier actif' : 'Bouclier en pause'}
                <span className={`w-1.5 h-1.5 rounded-full ${shieldStats.enabled ? 'bg-emerald-400' : 'bg-amber-400'}`} />
              </div>
              <div className="text-[10.5px] text-[var(--text-2)] font-medium mt-0.5 truncate">
                DNT + GPC · {shieldStats.userAgentProfile}
              </div>
            </div>
          </button>
        </section>

        {/* ---------- Accès rapides ---------- */}
        <section className="space-y-2.5">
          <div className="flex items-center justify-between">
            <h2 className="text-[10px] font-bold uppercase tracking-[0.16em] text-[var(--text-3)] flex items-center gap-1.5">
              <Compass className="w-3.5 h-3.5 text-[var(--accent)]" />
              Accès rapides
            </h2>
            <button
              type="button"
              onClick={() => setShowAddDial((v) => !v)}
              className="text-[11px] font-semibold text-[var(--accent)] hover:opacity-80 flex items-center gap-1 cursor-pointer"
            >
              <Plus className="w-3 h-3" />
              Ajouter
            </button>
          </div>

          {showAddDial && (
            <form onSubmit={handleAddDial} className="orakle-card p-3 flex flex-wrap items-center gap-2">
              <input
                type="text"
                placeholder="Nom (ex: GitLab)"
                value={newTitle}
                onChange={(e) => setNewTitle(e.target.value)}
                className="px-2.5 py-1.5 rounded-xl bg-[var(--fill)] border border-[var(--stroke)] text-[11.5px] text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
                required
              />
              <input
                type="text"
                placeholder="URL (ex: https://gitlab.com)"
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
                className="flex-1 min-w-[180px] px-2.5 py-1.5 rounded-xl bg-[var(--fill)] border border-[var(--stroke)] text-[11.5px] text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
                required
              />
              <button type="submit" className="btn-primary !py-1.5 !text-[11px]">
                Enregistrer
              </button>
            </form>
          )}

          <div className="grid grid-cols-3 sm:grid-cols-4 lg:grid-cols-8 gap-2.5">
            {speedDials.map((dial) => (
              <div
                key={dial.id}
                onClick={() => onNavigate(dial.url)}
                className="orakle-card group !relative p-3 flex flex-col items-center text-center gap-2 cursor-pointer"
              >
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onUpdateSpeedDials(speedDials.filter((d) => d.id !== dial.id));
                  }}
                  className="opacity-0 group-hover:opacity-100 !absolute top-1.5 right-1.5 p-1 rounded-md bg-black/30 text-[var(--text-3)] hover:text-red-400 transition-opacity cursor-pointer"
                  title="Supprimer"
                  aria-label={`Supprimer ${dial.title}`}
                >
                  <Trash2 className="w-2.5 h-2.5" />
                </button>
                <div
                  className="w-10 h-10 rounded-[14px] flex items-center justify-center text-[10.5px] font-extrabold text-white transition-transform group-hover:scale-105"
                  style={{
                    background: `linear-gradient(140deg, ${dial.color}, color-mix(in srgb, ${dial.color} 55%, #000 12%))`,
                    boxShadow: `0 8px 20px -8px ${dial.color}aa, inset 0 1px 0 rgba(255,255,255,0.35)`,
                  }}
                >
                  {dial.badge}
                </div>
                <div className="w-full min-w-0">
                  <div className="text-[11px] font-semibold truncate">{dial.title}</div>
                  <div className="text-[9px] text-[var(--text-3)] truncate">
                    {extractHostname(dial.url)}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </section>

        {/* ---------- Grille basse : reprise, notes, historique ---------- */}
        <section className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3 pb-8">
          {/* Sessions à reprendre */}
          <div className="orakle-card p-4 flex flex-col gap-2.5">
            <div className="flex items-center justify-between">
              <h3 className="text-[10px] font-bold uppercase tracking-[0.16em] text-[var(--text-3)] flex items-center gap-1.5">
                <Layers className="w-3.5 h-3.5 text-[var(--accent)]" />
                Reprendre
              </h3>
              <span className="text-[9.5px] text-[var(--text-3)]">{sessions.length} session(s)</span>
            </div>
            {sessions.length === 0 ? (
              <div className="flex-1 flex flex-col items-center justify-center text-center py-6 gap-1.5">
                <Layers className="w-6 h-6 opacity-30" />
                <p className="text-[10.5px] text-[var(--text-3)] max-w-[190px] leading-relaxed">
                  Enregistrez vos onglets en « session » pour les rouvrir plus tard en un clic.
                </p>
              </div>
            ) : (
              <div className="space-y-1.5">
                {sessions.slice(0, 4).map((s) => (
                  <button
                    key={s.id}
                    type="button"
                    onClick={() => onRestoreSession(s)}
                    className="w-full flex items-center gap-2 px-2.5 py-2 rounded-xl bg-[var(--fill)] hover:bg-[var(--fill-hover)] border border-transparent hover:border-[var(--stroke-strong)] transition-all text-left cursor-pointer group"
                  >
                    <span className="text-[13px]">{s.emoji}</span>
                    <span className="min-w-0 flex-1">
                      <span className="block text-[11px] font-semibold truncate">{s.name}</span>
                      <span className="block text-[9.5px] text-[var(--text-3)]">
                        {s.tabs.length} onglets
                      </span>
                    </span>
                    <Play className="w-3 h-3 text-[var(--text-3)] group-hover:text-[var(--accent)] transition-colors" />
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Notes */}
          <div className="orakle-card p-4 flex flex-col gap-2.5">
            <div className="flex items-center justify-between">
              <h3 className="text-[10px] font-bold uppercase tracking-[0.16em] text-[var(--text-3)] flex items-center gap-1.5">
                <FileText className="w-3.5 h-3.5 text-[var(--accent)]" />
                Scratchpad
              </h3>
              <span className="text-[9.5px] text-[var(--text-3)] flex items-center gap-1">
                <span className="w-1 h-1 rounded-full bg-emerald-400" />
                auto-sauvegardé
              </span>
            </div>
            <textarea
              value={scratchpad}
              onChange={(e) => onUpdateScratchpad(e.target.value)}
              placeholder="Notes, commandes, extraits, URLs à analyser…"
              rows={6}
              className="w-full flex-1 min-h-[110px] resize-none rounded-xl bg-[var(--fill)] border border-[var(--stroke)] p-2.5 text-[11px] font-mono leading-relaxed text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none focus:border-[var(--accent)]"
            />
          </div>

          {/* Historique récent */}
          <div className="orakle-card p-4 flex flex-col gap-2.5">
            <div className="flex items-center justify-between">
              <h3 className="text-[10px] font-bold uppercase tracking-[0.16em] text-[var(--text-3)] flex items-center gap-1.5">
                <Clock className="w-3.5 h-3.5 text-[var(--accent)]" />
                Récemment
              </h3>
              <span className="text-[9.5px] text-[var(--text-3)]">{recentHistory.length} entrées</span>
            </div>
            {recentHistory.length === 0 ? (
              <div className="flex-1 flex flex-col items-center justify-center text-center py-6 gap-1.5 text-[var(--text-3)]">
                <Globe className="w-6 h-6 opacity-30" />
                <p className="text-[10.5px]">Aucune page visitée récemment.</p>
              </div>
            ) : (
              <div className="space-y-0.5">
                {recentHistory.slice(0, 6).map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => onNavigate(item.url)}
                    className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-[var(--fill-hover)] text-left transition-colors cursor-pointer group"
                  >
                    <span className="w-1 h-1 rounded-full bg-[var(--text-3)] group-hover:bg-[var(--accent)] shrink-0" />
                    <span className="min-w-0 flex-1">
                      <span className="block text-[11px] font-medium truncate group-hover:text-[var(--accent)] transition-colors">
                        {item.title}
                      </span>
                      <span className="block text-[9px] text-[var(--text-3)] truncate">
                        {prettyUrl(item.url, 38)}
                      </span>
                    </span>
                    <ArrowUpRight className="w-3 h-3 text-[var(--text-3)] opacity-0 group-hover:opacity-100 shrink-0" />
                  </button>
                ))}
              </div>
            )}
          </div>
        </section>

        <footer className="pb-6 flex items-center justify-center gap-3 text-[10px] text-[var(--text-3)]">
          <span className="flex items-center gap-1">
            <BookOpen className="w-3 h-3" /> Ctrl+Shift+R — Mode Lecture
          </span>
          <span>·</span>
          <span className="flex items-center gap-1">
            <ExternalLink className="w-3 h-3" /> Ctrl+K — Commandes
          </span>
          <span>·</span>
          <span className="flex items-center gap-1">
            <ChevronRight className="w-3 h-3" /> F11 — Zen
          </span>
        </footer>
      </div>
    </div>
  );
};
