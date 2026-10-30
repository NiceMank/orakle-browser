import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  Search,
  Plus,
  Lock,
  Columns2,
  Shield,
  Moon,
  Sun,
  Layout,
  Terminal,
  Bookmark,
  Clock,
  Trash2,
  Globe,
  CornerDownLeft,
  BookOpen,
  Gauge,
  Cookie,
  Layers,
  Sparkles,
  Archive,
  Eye,
  Command,
} from 'lucide-react';
import { Tab } from '../types/browser';
import { prettyUrl, extractHostname, fuzzyScore } from '../utils/url';

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  tabs: Tab[];
  onSwitchTab: (id: string) => void;
  onNewTab: (url?: string, isPrivate?: boolean) => void;
  onToggleSplitView: () => void;
  onToggleLayout: () => void;
  onToggleTheme: () => void;
  onOpenShield: () => void;
  onOpenDevTools: () => void;
  onClearData: () => void;
  onOpenSidePanel: (panel: 'bookmarks' | 'history' | 'downloads' | 'notes' | 'settings' | 'privacy' | 'pulse' | 'spaces') => void;
  onOpenReader: () => void;
  onOpenFocus: () => void;
  onSaveSession: () => void;
  onToggleZen: () => void;
  isDark: boolean;
}

type Cmd = {
  id: string;
  label: string;
  hint?: string;
  group: string;
  icon: React.ReactNode;
  run: () => void;
};

export const CommandPalette: React.FC<CommandPaletteProps> = ({
  isOpen,
  onClose,
  tabs,
  onSwitchTab,
  onNewTab,
  onToggleSplitView,
  onToggleLayout,
  onToggleTheme,
  onOpenShield,
  onOpenDevTools,
  onClearData,
  onOpenSidePanel,
  onOpenReader,
  onOpenFocus,
  onSaveSession,
  onToggleZen,
  isDark,
}) => {
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const listRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (isOpen) {
      setQuery('');
      setIndex(0);
    }
  }, [isOpen]);

  const commands = useMemo<Cmd[]>(() => {
    const nav: Cmd[] = [
      { id: 'new-tab', label: 'Nouvel onglet', hint: 'Ctrl+T', group: 'Navigation', icon: <Plus className="w-3.5 h-3.5" />, run: () => onNewTab('orakle://newtab') },
      { id: 'new-private', label: 'Nouvel onglet Fantôme (privé)', hint: 'Ctrl+Shift+N', group: 'Navigation', icon: <Lock className="w-3.5 h-3.5" />, run: () => onNewTab('orakle://newtab', true) },
      { id: 'split', label: 'Basculer l’écran scindé', hint: 'Ctrl+\\', group: 'Navigation', icon: <Columns2 className="w-3.5 h-3.5" />, run: onToggleSplitView },
      { id: 'layout', label: 'Inverser la disposition des onglets', group: 'Navigation', icon: <Layout className="w-3.5 h-3.5" />, run: onToggleLayout },
      { id: 'zen', label: 'Mode Zen (plein écran)', hint: 'F11', group: 'Navigation', icon: <Eye className="w-3.5 h-3.5" />, run: onToggleZen },
    ];

    const tools: Cmd[] = [
      { id: 'reader', label: 'Ouvrir le Mode Lecture', hint: 'Ctrl+Shift+R', group: 'Outils', icon: <BookOpen className="w-3.5 h-3.5" />, run: onOpenReader },
      { id: 'focus', label: 'Démarrer une session Focus', group: 'Outils', icon: <Sparkles className="w-3.5 h-3.5" />, run: onOpenFocus },
      { id: 'session', label: 'Enregistrer la session d’onglets', group: 'Outils', icon: <Archive className="w-3.5 h-3.5" />, run: onSaveSession },
      { id: 'shield', label: 'Orakle Shield & audit OWASP', group: 'Sécurité', icon: <Shield className="w-3.5 h-3.5" />, run: onOpenShield },
      { id: 'privacy', label: 'Inspecteur de cookies & traces', group: 'Sécurité', icon: <Cookie className="w-3.5 h-3.5" />, run: () => onOpenSidePanel('privacy') },
      { id: 'pulse', label: 'Orakle Pulse — latence réseau', group: 'Sécurité', icon: <Gauge className="w-3.5 h-3.5" />, run: () => onOpenSidePanel('pulse') },
      { id: 'clear', label: 'Purger toutes les données de navigation', group: 'Sécurité', icon: <Trash2 className="w-3.5 h-3.5" />, run: onClearData },
    ];

    const panels: Cmd[] = [
      { id: 'bm', label: 'Panneau Favoris', hint: 'Ctrl+B', group: 'Panneaux', icon: <Bookmark className="w-3.5 h-3.5" />, run: () => onOpenSidePanel('bookmarks') },
      { id: 'hist', label: 'Panneau Historique', hint: 'Ctrl+H', group: 'Panneaux', icon: <Clock className="w-3.5 h-3.5" />, run: () => onOpenSidePanel('history') },
      { id: 'dl', label: 'Panneau Téléchargements', group: 'Panneaux', icon: <Globe className="w-3.5 h-3.5" />, run: () => onOpenSidePanel('downloads') },
      { id: 'notes', label: 'Panneau Notes / Scratchpad', group: 'Panneaux', icon: <BookOpen className="w-3.5 h-3.5" />, run: () => onOpenSidePanel('notes') },
      { id: 'sessions', label: 'Panneau Sessions de travail', group: 'Panneaux', icon: <Layers className="w-3.5 h-3.5" />, run: () => onOpenSidePanel('spaces') },
      { id: 'settings', label: 'Panneau Réglages', group: 'Panneaux', icon: <Terminal className="w-3.5 h-3.5" />, run: () => onOpenSidePanel('settings') },
      { id: 'theme', label: isDark ? 'Passer au thème clair' : 'Passer au thème sombre', group: 'Apparence', icon: isDark ? <Sun className="w-3.5 h-3.5" /> : <Moon className="w-3.5 h-3.5" />, run: onToggleTheme },
      { id: 'devtools', label: 'Inspecteur DevTools', hint: 'F12', group: 'Outils', icon: <Terminal className="w-3.5 h-3.5" />, run: onOpenDevTools },
    ];

    const tabCmds: Cmd[] = tabs
      .filter((t) => t.url !== 'orakle://newtab')
      .slice(0, 8)
      .map((t) => ({
        id: `tab-${t.id}`,
        label: t.title || extractHostname(t.url),
        hint: prettyUrl(t.url, 34),
        group: 'Onglets ouverts',
        icon: <Globe className="w-3.5 h-3.5" />,
        run: () => onSwitchTab(t.id),
      }));

    return [...nav, ...tools, ...panels, ...tabCmds];
  }, [
    tabs,
    isDark,
    onNewTab,
    onToggleSplitView,
    onToggleLayout,
    onToggleZen,
    onOpenReader,
    onOpenFocus,
    onSaveSession,
    onOpenShield,
    onOpenSidePanel,
    onClearData,
    onToggleTheme,
    onOpenDevTools,
    onSwitchTab,
  ]);

  const filtered = useMemo(() => {
    if (!query.trim()) return commands;
    return commands
      .map((c) => ({ c, score: fuzzyScore(query, `${c.label} ${c.group} ${c.hint || ''}`) }))
      .filter((x) => x.score > 0)
      .sort((a, b) => b.score - a.score)
      .map((x) => x.c);
  }, [commands, query]);

  useEffect(() => setIndex(0), [query]);

  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-idx="${index}"]`);
    el?.scrollIntoView({ block: 'nearest' });
  }, [index]);

  if (!isOpen) return null;

  const grouped = filtered.reduce<Record<string, Cmd[]>>((acc, c) => {
    (acc[c.group] = acc[c.group] || []).push(c);
    return acc;
  }, {});

  let flatCounter = -1;

  const execute = (cmd: Cmd) => {
    cmd.run();
    onClose();
  };

  return (
    <div
      className="fixed inset-0 z-[70] flex items-start justify-center pt-[12vh] animate-overlay"
      style={{ background: 'rgba(3,4,8,0.62)' }}
      onClick={onClose}
    >
      <div
        className="w-[600px] max-w-[92vw] glass-panel-strong rounded-[26px] overflow-hidden animate-pop"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-3 px-4 py-3.5 border-b border-[var(--stroke)]">
          <Command className="w-4 h-4 text-[var(--accent)] shrink-0" />
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'ArrowDown') {
                e.preventDefault();
                setIndex((i) => Math.min(filtered.length - 1, i + 1));
              } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                setIndex((i) => Math.max(0, i - 1));
              } else if (e.key === 'Enter') {
                e.preventDefault();
                const cmd = filtered[index];
                if (cmd) execute(cmd);
              } else if (e.key === 'Escape') {
                onClose();
              }
            }}
            placeholder="Que voulez-vous faire ? (onglets, outils, réglages…)"
            className="w-full bg-transparent text-[13.5px] text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none focus-visible:outline-none"
          />
          <kbd className="kbd shrink-0">Échap</kbd>
        </div>

        <div ref={listRef} className="max-h-[46vh] overflow-y-auto py-1.5">
          {Object.entries(grouped).map(([group, items]) => (
            <div key={group} className="px-1.5">
              <div className="px-2.5 pt-2 pb-1 text-[9.5px] font-bold uppercase tracking-[0.16em] text-[var(--text-3)]">
                {group}
              </div>
              {items.map((cmd) => {
                flatCounter += 1;
                const myIdx = flatCounter;
                return (
                  <button
                    key={cmd.id}
                    data-idx={myIdx}
                    type="button"
                    onMouseEnter={() => setIndex(myIdx)}
                    onClick={() => execute(cmd)}
                    className={`w-full flex items-center gap-2.5 px-2.5 py-2 rounded-xl text-left transition-colors cursor-pointer ${
                      myIdx === index ? 'bg-[var(--accent-soft)]' : 'hover:bg-[var(--fill)]'
                    }`}
                  >
                    <span className="w-6 h-6 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] flex items-center justify-center text-[var(--text-2)] shrink-0">
                      {cmd.icon}
                    </span>
                    <span className="text-[12.5px] font-medium text-[var(--text)] flex-1 truncate">
                      {cmd.label}
                    </span>
                    {cmd.hint && <span className="text-[10px] text-[var(--text-3)] shrink-0">{cmd.hint}</span>}
                    {myIdx === index && <CornerDownLeft className="w-3.5 h-3.5 text-[var(--accent)] shrink-0" />}
                  </button>
                );
              })}
            </div>
          ))}
          {filtered.length === 0 && (
            <div className="px-4 py-8 text-center text-[11.5px] text-[var(--text-3)]">
              Aucune commande ne correspond à « {query} ».
            </div>
          )}
        </div>

        <div className="flex items-center justify-between px-4 py-2 border-t border-[var(--stroke)] bg-[var(--fill)] text-[10px] text-[var(--text-3)]">
          <span className="flex items-center gap-1.5">
            <Search className="w-3 h-3" />
            {filtered.length} commande(s)
          </span>
          <span className="flex items-center gap-1">
            <kbd className="kbd">↑</kbd>
            <kbd className="kbd">↓</kbd>
            naviguer
          </span>
        </div>
      </div>
    </div>
  );
};
