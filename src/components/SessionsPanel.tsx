import React, { useState } from 'react';
import { Archive, Plus, Trash2, Play, Layers, FolderPlus, X } from 'lucide-react';
import { SavedSession, Tab } from '../types/browser';
import { prettyUrl, relativeTime } from '../utils/url';

interface SessionsPanelProps {
  tabs: Tab[];
  sessions: SavedSession[];
  onSaveSession: (session: { name: string; emoji: string; tabs: { title: string; url: string }[] }) => void;
  onDeleteSession: (id: string) => void;
  onRestoreSession: (session: SavedSession) => void;
  onOpenUrls: (urls: string[]) => void;
}

const EMOJIS = ['🗂️', '🚀', '🧪', '📊', '🎧', '📚', '🔥', '🧠', '🛠️', '🌙'];

/**
 * Sessions de travail — capture/restauration d'un jeu d'onglets complet.
 */
export const SessionsPanel: React.FC<SessionsPanelProps> = ({
  tabs,
  sessions,
  onSaveSession,
  onDeleteSession,
  onRestoreSession,
  onOpenUrls,
}) => {
  const [name, setName] = useState('');
  const [emoji, setEmoji] = useState('🗂️');
  const [creating, setCreating] = useState(false);

  const capturable = tabs.filter((t) => t.url !== 'orakle://newtab');

  const save = () => {
    if (!name.trim() || capturable.length === 0) return;
    onSaveSession({
      name: name.trim(),
      emoji,
      tabs: capturable.map((t) => ({ title: t.title, url: t.url })),
    });
    setName('');
    setCreating(false);
  };

  return (
    <div className="space-y-3">
      <div className="orakle-card p-3.5 space-y-2.5">
        <div className="flex items-center gap-2">
          <Archive className="w-4 h-4 text-[var(--accent)]" />
          <span className="text-xs font-bold text-[var(--text)]">Sessions de travail</span>
          <span className="chip ml-auto !text-[9.5px]">{sessions.length}</span>
        </div>
        <p className="text-[10.5px] leading-relaxed text-[var(--text-2)]">
          Capturez vos {capturable.length} onglet(s) actifs en une session nommée, puis restaurez-la
          plus tard en un clic — comme un « snapshot » de contexte.
        </p>

        {!creating ? (
          <button
            type="button"
            onClick={() => setCreating(true)}
            disabled={capturable.length === 0}
            className="btn-primary w-full !py-2 disabled:opacity-40"
          >
            <Plus className="w-3.5 h-3.5" />
            Capturer la session actuelle
          </button>
        ) : (
          <div className="space-y-2">
            <input
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && save()}
              placeholder="Nom de la session (ex: Audit client X)"
              className="w-full px-2.5 py-2 rounded-xl bg-[var(--fill)] border border-[var(--stroke)] text-[11.5px] text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
            />
            <div className="flex flex-wrap gap-1">
              {EMOJIS.map((e) => (
                <button
                  key={e}
                  type="button"
                  onClick={() => setEmoji(e)}
                  className={`w-6 h-6 rounded-md text-[12px] flex items-center justify-center cursor-pointer ${
                    emoji === e ? 'bg-[var(--accent-soft)] ring-1 ring-[var(--accent)]' : 'hover:bg-[var(--fill)]'
                  }`}
                >
                  {e}
                </button>
              ))}
            </div>
            <div className="flex gap-1.5">
              <button type="button" onClick={save} className="btn-primary flex-1 !py-1.5 !text-[11px]">
                <FolderPlus className="w-3 h-3" />
                Enregistrer
              </button>
              <button type="button" onClick={() => setCreating(false)} className="btn-glass !py-1.5">
                <X className="w-3 h-3" />
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="space-y-1.5">
        {sessions.length === 0 && (
          <div className="orakle-card p-6 text-center text-[11px] text-[var(--text-3)]">
            Aucune session enregistrée.
          </div>
        )}
        {sessions.map((s) => (
          <div key={s.id} className="orakle-card !rounded-xl p-2.5 space-y-1.5">
            <div className="flex items-center gap-2">
              <span className="text-[14px]">{s.emoji}</span>
              <div className="min-w-0 flex-1">
                <div className="text-[11.5px] font-bold text-[var(--text)] truncate">{s.name}</div>
                <div className="text-[9.5px] text-[var(--text-3)]">
                  {s.tabs.length} onglets · {relativeTime(s.createdAt)}
                </div>
              </div>
              <button
                type="button"
                onClick={() => onRestoreSession(s)}
                className="icon-btn w-6 h-6"
                title="Restaurer la session"
              >
                <Play className="w-3 h-3" />
              </button>
              <button
                type="button"
                onClick={() => onDeleteSession(s.id)}
                className="icon-btn w-6 h-6 hover:text-red-400"
                title="Supprimer"
              >
                <Trash2 className="w-3 h-3" />
              </button>
            </div>

            <div className="space-y-0.5 max-h-24 overflow-y-auto">
              {s.tabs.slice(0, 8).map((t, i) => (
                <div key={`${s.id}-${i}`} className="flex items-center gap-1.5 text-[9.5px] text-[var(--text-3)]">
                  <Layers className="w-2.5 h-2.5 shrink-0" />
                  <span className="truncate">{prettyUrl(t.url, 40)}</span>
                </div>
              ))}
            </div>

            <button
              type="button"
              onClick={() => onOpenUrls(s.tabs.map((t) => t.url))}
              className="w-full py-1 rounded-lg text-[10px] font-semibold bg-[var(--fill)] hover:bg-[var(--fill-hover)] text-[var(--text-2)] transition-colors cursor-pointer"
            >
              Ouvrir tous les onglets en arrière-plan
            </button>
          </div>
        ))}
      </div>
    </div>
  );
};
