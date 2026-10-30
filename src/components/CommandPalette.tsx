import React, { useState, useEffect } from 'react';
import {
  Search,
  Plus,
  Lock,
  Columns,
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
} from 'lucide-react';
import { Tab } from '../types/browser';

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
  onOpenSidePanel: (panel: 'bookmarks' | 'history' | 'downloads' | 'notes' | 'settings') => void;
}

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
}) => {
  const [query, setQuery] = useState('');

  useEffect(() => {
    if (isOpen) {
      setQuery('');
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const commands = [
    {
      id: 'cmd-new-tab',
      label: 'Nouvel Onglet',
      shortcut: 'Ctrl+T',
      icon: Plus,
      action: () => onNewTab('orakle://newtab', false),
    },
    {
      id: 'cmd-private-tab',
      label: 'Nouvel Onglet Fantôme (Navigation Privée Isolée)',
      shortcut: 'Ctrl+Shift+N',
      icon: Lock,
      action: () => onNewTab('orakle://newtab', true),
    },
    {
      id: 'cmd-split-view',
      label: 'Basculer le Mode Écran Scindé (Split View)',
      shortcut: 'Ctrl+\\',
      icon: Columns,
      action: onToggleSplitView,
    },
    {
      id: 'cmd-shield',
      label: 'Ouvrir Orakle Shield & Audit de Sécurité OWASP',
      shortcut: 'Alt+S',
      icon: Shield,
      action: onOpenShield,
    },
    {
      id: 'cmd-layout',
      label: 'Basculer Onglets Horizontaux / Barre Verticale (Style Arc)',
      shortcut: 'Ctrl+Shift+L',
      icon: Layout,
      action: onToggleLayout,
    },
    {
      id: 'cmd-theme',
      label: 'Basculer Thème Sombre / Clair',
      shortcut: 'Ctrl+Shift+D',
      icon: Moon,
      action: onToggleTheme,
    },
    {
      id: 'cmd-devtools',
      label: 'Ouvrir l’Inspecteur Web (DevTools)',
      shortcut: 'F12',
      icon: Terminal,
      action: onOpenDevTools,
    },
    {
      id: 'cmd-bookmarks',
      label: 'Gérer les Favoris',
      shortcut: 'Ctrl+B',
      icon: Bookmark,
      action: () => onOpenSidePanel('bookmarks'),
    },
    {
      id: 'cmd-history',
      label: 'Consulter l’Historique',
      shortcut: 'Ctrl+H',
      icon: Clock,
      action: () => onOpenSidePanel('history'),
    },
    {
      id: 'cmd-clear',
      label: 'Purger les traces, cookies et cache de navigation',
      shortcut: 'Ctrl+Shift+Suppr',
      icon: Trash2,
      action: onClearData,
    },
  ];

  const filteredCommands = commands.filter((c) =>
    c.label.toLowerCase().includes(query.toLowerCase())
  );

  const matchingTabs = tabs.filter(
    (t) =>
      t.title.toLowerCase().includes(query.toLowerCase()) ||
      t.url.toLowerCase().includes(query.toLowerCase())
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-24 bg-black/50 backdrop-blur-xs"
      onClick={onClose}
    >
      <div
        className="glass-panel-strong w-full max-w-xl rounded-3xl overflow-hidden shadow-2xl animate-in fade-in zoom-in-95 duration-150"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-3 px-4 py-3.5 border-b border-[var(--stroke)]">
          <Search className="w-5 h-5 text-[var(--accent)]" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Escape') onClose();
              if (e.key === 'Enter' && filteredCommands.length > 0) {
                filteredCommands[0].action();
                onClose();
              }
            }}
            placeholder="Tapez une commande ou recherchez un onglet ouvert..."
            className="w-full bg-transparent text-sm text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none"
            autoFocus
          />
          <span className="px-2 py-0.5 rounded-md text-[10px] font-mono bg-[var(--fill-strong)] text-[var(--text-2)]">
            ESC
          </span>
        </div>

        <div className="max-h-96 overflow-y-auto p-2 space-y-3">
          {matchingTabs.length > 0 && (
            <div>
              <div className="px-3 py-1 text-[10px] font-bold uppercase tracking-wider text-[var(--text-3)]">
                Onglets Ouverts ({matchingTabs.length})
              </div>
              {matchingTabs.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  onClick={() => {
                    onSwitchTab(t.id);
                    onClose();
                  }}
                  className="w-full flex items-center justify-between gap-3 px-3 py-2 rounded-xl hover:bg-[var(--fill-hover)] text-left transition-colors cursor-pointer"
                >
                  <div className="flex items-center gap-2.5 min-w-0">
                    <Globe className="w-4 h-4 text-[var(--accent)] shrink-0" />
                    <span className="text-xs font-medium text-[var(--text)] truncate">
                      {t.title}
                    </span>
                  </div>
                  <span className="text-[10px] text-[var(--text-3)] flex items-center gap-1 shrink-0">
                    Basculer <CornerDownLeft className="w-3 h-3" />
                  </span>
                </button>
              ))}
            </div>
          )}

          <div>
            <div className="px-3 py-1 text-[10px] font-bold uppercase tracking-wider text-[var(--text-3)]">
              Actions Rapides Orakle
            </div>
            {filteredCommands.map((cmd) => {
              const Icon = cmd.icon;
              return (
                <button
                  key={cmd.id}
                  type="button"
                  onClick={() => {
                    cmd.action();
                    onClose();
                  }}
                  className="w-full flex items-center justify-between gap-3 px-3 py-2.5 rounded-xl hover:bg-[var(--fill-hover)] text-left transition-colors cursor-pointer group"
                >
                  <div className="flex items-center gap-3">
                    <div className="w-7 h-7 rounded-lg bg-[var(--fill-strong)] flex items-center justify-center text-[var(--text-2)] group-hover:text-[var(--accent)]">
                      <Icon className="w-4 h-4" />
                    </div>
                    <span className="text-xs font-medium text-[var(--text)]">{cmd.label}</span>
                  </div>
                  <kbd className="px-2 py-0.5 rounded-md text-[10px] font-mono bg-[var(--fill)] border border-[var(--stroke)] text-[var(--text-2)]">
                    {cmd.shortcut}
                  </kbd>
                </button>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
};
