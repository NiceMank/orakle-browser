import React, { useState } from 'react';
import {
  X,
  Plus,
  Volume2,
  VolumeX,
  RotateCw,
  EyeOff,
  Compass,
  Pin,
  Moon,
  Columns2,
  Sun,
  Sparkles,
} from 'lucide-react';
import { Tab } from '../types/browser';
import { extractHostname } from '../utils/url';

interface TabStripProps {
  tabs: Tab[];
  activeTabId: string;
  splitTabId: string | null;
  onSelect: (id: string) => void;
  onClose: (id: string, e?: React.MouseEvent) => void;
  onNewTab: (privateTab?: boolean) => void;
  onToggleMute: (id: string, e?: React.MouseEvent) => void;
  onTogglePin: (id: string, e?: React.MouseEvent) => void;
  onSleepTab: (id: string) => void;
  onSplitWith: (id: string) => void;
}

/** Barre d'onglets horizontale en capsules flottantes */
export const TabStrip: React.FC<TabStripProps> = ({
  tabs,
  activeTabId,
  splitTabId,
  onSelect,
  onClose,
  onNewTab,
  onToggleMute,
  onTogglePin,
  onSleepTab,
  onSplitWith,
}) => {
  const [menuFor, setMenuFor] = useState<string | null>(null);

  const ordered = [...tabs].sort((a, b) => Number(b.isPinned) - Number(a.isPinned));

  return (
    <div className="flex items-center gap-1.5 flex-1 min-w-0 overflow-x-auto no-scrollbar">
      {ordered.map((tab) => {
        const isActive = tab.id === activeTabId;
        const isSplit = tab.id === splitTabId;
        const isNewTab = tab.url === 'orakle://newtab';

        return (
          <div key={tab.id} className="!relative shrink-0">
            <div
              className={`tab-pill ${tab.isPinned ? '!px-2' : 'max-w-[210px]'}`}
              data-active={isActive}
              data-private={tab.isPrivate}
              onClick={() => onSelect(tab.id)}
              onDoubleClick={() => onTogglePin(tab.id)}
              onAuxClick={(e) => {
                if (e.button === 1) onClose(tab.id, e);
              }}
              onContextMenu={(e) => {
                e.preventDefault();
                setMenuFor(menuFor === tab.id ? null : tab.id);
              }}
              title={`${tab.title}\n${tab.url}\nDouble-clic : épingler · clic droit : options`}
            >
              {isSplit && (
                <span className="absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full bg-[var(--accent)] ring-2 ring-[var(--bg)]" />
              )}

              {tab.isLoading ? (
                <RotateCw className="w-3.5 h-3.5 animate-spin text-[var(--accent)] shrink-0" />
              ) : tab.isPrivate ? (
                <EyeOff className="w-3.5 h-3.5 text-[var(--c-cyber)] shrink-0" />
              ) : tab.favicon ? (
                <img
                  src={tab.favicon}
                  alt=""
                  className="w-3.5 h-3.5 rounded-[4px] shrink-0 object-contain"
                  onError={(e) => {
                    (e.target as HTMLImageElement).style.visibility = 'hidden';
                  }}
                />
              ) : (
                <Compass className="w-3.5 h-3.5 text-[var(--accent)] shrink-0" />
              )}

              {!tab.isPinned && (
                <span className="text-[11.5px] font-medium truncate flex-1 min-w-0">
                  {tab.title || extractHostname(tab.url)}
                </span>
              )}

              {isNewTab && !tab.isPinned && (
                <Sparkles className="w-3 h-3 text-[var(--accent)] shrink-0 opacity-60" />
              )}

              {(tab.isAudioPlaying || tab.isMuted) && (
                <button
                  type="button"
                  onClick={(e) => onToggleMute(tab.id, e)}
                  className="p-0.5 rounded-md hover:bg-[var(--fill-strong)] shrink-0 cursor-pointer"
                  title={tab.isMuted ? 'Réactiver le son' : 'Couper le son'}
                >
                  {tab.isMuted ? (
                    <VolumeX className="w-3 h-3 text-red-400" />
                  ) : (
                    <Volume2 className="w-3 h-3 text-[var(--accent)]" />
                  )}
                </button>
              )}

              {tab.isPinned && <Pin className="w-2.5 h-2.5 text-[var(--accent)] shrink-0" />}

              {!tab.isPinned && (
                <button
                  type="button"
                  onClick={(e) => onClose(tab.id, e)}
                  className="p-0.5 rounded-md hover:bg-[var(--fill-strong)] text-[var(--text-3)] hover:text-[var(--text)] transition-all cursor-pointer shrink-0 opacity-45 hover:opacity-100" 
                  aria-label={`Fermer ${tab.title}`}
                >
                  <X className="w-3 h-3" />
                </button>
              )}
            </div>

            {/* Menu contextuel d'onglet */}
            {menuFor === tab.id && (
              <>
                <div className="fixed inset-0 z-40" onClick={() => setMenuFor(null)} />
                <div className="!absolute top-[calc(100%+6px)] left-0 z-50 glass-panel-strong rounded-xl p-1.5 w-52 animate-pop">
                  {[
                    {
                      icon: <Pin className="w-3.5 h-3.5" />,
                      label: tab.isPinned ? 'Désépingler' : 'Épingler',
                      action: () => onTogglePin(tab.id),
                    },
                    {
                      icon: <Columns2 className="w-3.5 h-3.5" />,
                      label: 'Ouvrir en écran scindé',
                      action: () => onSplitWith(tab.id),
                    },
                    {
                      icon: <Moon className="w-3.5 h-3.5" />,
                      label: 'Mettre en veille (libère la RAM)',
                      action: () => onSleepTab(tab.id),
                    },
                    {
                      icon: <VolumeX className="w-3.5 h-3.5" />,
                      label: tab.isMuted ? 'Réactiver le son' : 'Couper le son',
                      action: () => onToggleMute(tab.id),
                    },
                    {
                      icon: <X className="w-3.5 h-3.5" />,
                      label: 'Fermer l’onglet',
                      action: () => onClose(tab.id),
                    },
                  ].map((item) => (
                    <button
                      key={item.label}
                      type="button"
                      onClick={() => {
                        item.action();
                        setMenuFor(null);
                      }}
                      className="w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-[11.5px] font-medium text-[var(--text-2)] hover:text-[var(--text)] hover:bg-[var(--fill-hover)] transition-colors cursor-pointer text-left"
                    >
                      {item.icon}
                      {item.label}
                    </button>
                  ))}
                </div>
              </>
            )}
          </div>
        );
      })}

      <button
        type="button"
        onClick={() => onNewTab(false)}
        className="app-no-drag icon-btn w-7 h-7 shrink-0 rounded-full"
        title="Nouvel onglet (Ctrl+T)"
        aria-label="Nouvel onglet"
      >
        <Plus className="w-4 h-4" />
      </button>
      <button
        type="button"
        onClick={() => onNewTab(true)}
        className="app-no-drag icon-btn w-7 h-7 shrink-0 rounded-full text-[var(--c-cyber)] hover:bg-purple-500/15"
        title="Onglet Fantôme privé (Ctrl+Shift+N)"
        aria-label="Onglet privé"
      >
        <EyeOff className="w-3.5 h-3.5" />
      </button>
    </div>
  );
};
