import React, { useState } from 'react';
import { Plus, Settings2, Check, Trash2 } from 'lucide-react';
import { Space } from '../types/browser';

interface SpaceRailProps {
  spaces: Space[];
  activeSpaceId: string;
  tabCounts: Record<string, number>;
  onSelect: (id: string) => void;
  onSaveSpaces: (spaces: Space[]) => void;
}

const EMOJIS = ['🌙', '💼', '🛰️', '🚀', '🎧', '📚', '🧪', '🛡️', '🌿', '🔥'];
const COLORS = ['#0a84ff', '#8b5cf6', '#10b981', '#f59e0b', '#ec4899', '#38bdf8', '#f43f5e'];

/**
 * Rail d'espaces de travail (Spaces) — chaque espace isole ses onglets.
 * Inspiré d'Arc, mais avec compteur d'onglets et édition en ligne.
 */
export const SpaceRail: React.FC<SpaceRailProps> = ({
  spaces,
  activeSpaceId,
  tabCounts,
  onSelect,
  onSaveSpaces,
}) => {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Space | null>(null);

  const commit = () => {
    if (!draft) return;
    const exists = spaces.some((s) => s.id === draft.id);
    const next = exists
      ? spaces.map((s) => (s.id === draft.id ? draft : s))
      : [...spaces, draft];
    onSaveSpaces(next);
    setDraft(null);
  };

  return (
    <aside className="app-no-drag w-14 shrink-0 flex flex-col items-center gap-2 py-3 border-r border-[var(--stroke)] glass-thin z-20">
      <div className="flex flex-col items-center gap-2.5">
        {spaces.map((space) => {
          const active = space.id === activeSpaceId;
          const count = tabCounts[space.id] || 0;
          return (
            <button
              key={space.id}
              type="button"
              onClick={() => onSelect(space.id)}
              className="space-orb"
              data-active={active}
              style={{
                background: active
                  ? `linear-gradient(135deg, ${space.color}, color-mix(in srgb, ${space.color} 45%, #ffffff 20%))`
                  : `color-mix(in srgb, ${space.color} 22%, transparent)`,
                color: active ? '#fff' : space.color,
              }}
              title={`${space.name} — ${count} onglet(s)`}
            >
              <span className="text-[13px] leading-none">{space.emoji}</span>
              {count > 0 && (
                <span
                  className="absolute -bottom-1 -right-1 min-w-[15px] h-[15px] px-1 rounded-full text-[8.5px] font-bold flex items-center justify-center bg-[var(--bg-elev-2)] border border-[var(--stroke-strong)] text-[var(--text-2)]"
                >
                  {count}
                </span>
              )}
            </button>
          );
        })}

        <button
          type="button"
          onClick={() => {
            setDraft({
              id: `space-${Date.now()}`,
              name: 'Nouvel espace',
              emoji: EMOJIS[spaces.length % EMOJIS.length],
              color: COLORS[spaces.length % COLORS.length],
              createdAt: Date.now(),
            });
          }}
          className="w-[30px] h-[30px] rounded-full border border-dashed border-[var(--stroke-strong)] text-[var(--text-3)] hover:text-[var(--accent)] hover:border-[var(--accent)] transition-colors flex items-center justify-center cursor-pointer"
          title="Créer un espace"
        >
          <Plus className="w-3.5 h-3.5" />
        </button>
      </div>

      <div className="flex-1" />

      <button
        type="button"
        onClick={() => setEditing((e) => !e)}
        className={`w-[30px] h-[30px] rounded-full flex items-center justify-center transition-colors cursor-pointer ${
          editing
            ? 'bg-[var(--accent-soft)] text-[var(--accent)]'
            : 'text-[var(--text-3)] hover:text-[var(--text)] hover:bg-[var(--fill)]'
        }`}
        title="Gérer les espaces"
      >
        <Settings2 className="w-3.5 h-3.5" />
      </button>

      {/* Panneau d'édition */}
      {(editing || draft) && (
        <div className="!fixed left-14 top-14 z-50 w-64 glass-panel-strong rounded-2xl p-3 animate-pop">
          <div className="text-[10px] font-bold uppercase tracking-[0.14em] text-[var(--text-3)] mb-2">
            Espaces de travail
          </div>

          <div className="space-y-1.5 max-h-64 overflow-y-auto">
            {spaces.map((space) => (
              <div
                key={space.id}
                className="flex items-center gap-2 px-2 py-1.5 rounded-lg bg-[var(--fill)]"
              >
                <span className="text-[13px]">{space.emoji}</span>
                <input
                  value={space.name}
                  onChange={(e) =>
                    onSaveSpaces(
                      spaces.map((s) => (s.id === space.id ? { ...s, name: e.target.value } : s))
                    )
                  }
                  className="flex-1 min-w-0 bg-transparent text-[11.5px] font-semibold text-[var(--text)] focus:outline-none"
                />
                <input
                  type="color"
                  value={space.color}
                  onChange={(e) =>
                    onSaveSpaces(
                      spaces.map((s) => (s.id === space.id ? { ...s, color: e.target.value } : s))
                    )
                  }
                  className="w-5 h-5 rounded border border-[var(--stroke)] bg-transparent cursor-pointer"
                />
                <button
                  type="button"
                  onClick={() => onSaveSpaces(spaces.filter((s) => s.id !== space.id))}
                  className="text-[var(--text-3)] hover:text-red-400 cursor-pointer"
                  title="Supprimer l'espace"
                  disabled={spaces.length <= 1}
                >
                  <Trash2 className="w-3 h-3" />
                </button>
              </div>
            ))}
          </div>

          {draft && (
            <div className="mt-2.5 pt-2.5 border-t border-[var(--stroke)] space-y-2">
              <input
                autoFocus
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                placeholder="Nom de l'espace"
                className="w-full px-2 py-1.5 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] text-[11.5px] text-[var(--text)] focus:outline-none focus:border-[var(--accent)]"
              />
              <div className="flex flex-wrap gap-1">
                {EMOJIS.map((e) => (
                  <button
                    key={e}
                    type="button"
                    onClick={() => setDraft({ ...draft, emoji: e })}
                    className={`w-6 h-6 rounded-md text-[12px] flex items-center justify-center cursor-pointer ${
                      draft.emoji === e ? 'bg-[var(--accent-soft)] ring-1 ring-[var(--accent)]' : 'hover:bg-[var(--fill)]'
                    }`}
                  >
                    {e}
                  </button>
                ))}
              </div>
              <div className="flex gap-1">
                {COLORS.map((c) => (
                  <button
                    key={c}
                    type="button"
                    onClick={() => setDraft({ ...draft, color: c })}
                    className="w-5 h-5 rounded-full cursor-pointer border-2"
                    style={{
                      backgroundColor: c,
                      borderColor: draft.color === c ? '#fff' : 'transparent',
                    }}
                  />
                ))}
              </div>
              <button
                type="button"
                onClick={() => {
                  commit();
                  setEditing(false);
                }}
                className="btn-primary w-full !py-1.5 !text-[11px]"
              >
                <Check className="w-3 h-3" />
                Créer l’espace
              </button>
            </div>
          )}
        </div>
      )}
    </aside>
  );
};
