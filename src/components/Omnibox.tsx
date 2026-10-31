import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  Lock,
  Unlock,
  Sparkles,
  ShieldCheck,
  ShieldAlert,
  Star,
  Search,
  History,
  CornerDownLeft,
  Globe,
  Command,
  Zap,
  ScanLine,
  Gauge,
} from 'lucide-react';
import { BookmarkItem, HistoryItem, SearchEngineId, Tab } from '../types/browser';
import { BANG_SHORTCUTS, SEARCH_ENGINES, fuzzyScore, parseOmniboxInput, prettyUrl } from '../utils/url';

interface OmniboxProps {
  value: string;
  onChange: (val: string) => void;
  onSubmit: (val: string) => void;
  onOpenShield: () => void;
  onOpenPalette: () => void;
  securityState: 'secure' | 'insecure' | 'internal';
  zoomFactor: number;
  onResetZoom: () => void;
  searchEngine: SearchEngineId;
  bookmarks: BookmarkItem[];
  history: HistoryItem[];
  tabs: Tab[];
  isBookmarked: boolean;
  onToggleBookmark: () => void;
  shieldBlocked: number;
  shieldEnabled: boolean;
  focused: boolean;
  onFocusChange: (focused: boolean) => void;
  inputRef: React.RefObject<HTMLInputElement>;
}

type Suggestion = {
  id: string;
  icon: React.ReactNode;
  title: string;
  subtitle?: string;
  badge?: string;
  action: () => void;
};

/**
 * Omnibox « Aurora » : capsule flottante + panneau de suggestions
 * enrichi (bangs, favoris, historique, actions rapides).
 */
export const Omnibox: React.FC<OmniboxProps> = ({
  value,
  onChange,
  onSubmit,
  onOpenShield,
  onOpenPalette,
  securityState,
  zoomFactor,
  onResetZoom,
  searchEngine,
  bookmarks,
  history,
  tabs,
  isBookmarked,
  onToggleBookmark,
  shieldBlocked,
  shieldEnabled,
  focused,
  onFocusChange,
  inputRef,
}) => {
  const [highlight, setHighlight] = useState(0);
  const wrapRef = useRef<HTMLDivElement | null>(null);

  const parsed = useMemo(
    () => parseOmniboxInput(value, searchEngine),
    [value, searchEngine]
  );

  const suggestions = useMemo<Suggestion[]>(() => {
    const q = value.trim().toLowerCase();
    const out: Suggestion[] = [];

    // 1. Action principale (navigation ou recherche)
    if (value.trim()) {
      out.push({
        id: 'primary',
        icon:
          parsed.kind === 'url' ? (
            <Globe className="w-4 h-4 text-[var(--accent)]" />
          ) : parsed.kind === 'bang' ? (
            <Zap className="w-4 h-4 text-amber-400" />
          ) : (
            <Search className="w-4 h-4 text-[var(--accent)]" />
          ),
        title: parsed.label,
        subtitle: parsed.kind === 'url' ? 'Ouvrir l’adresse' : 'Lancer la recherche',
        badge: parsed.kind === 'url' ? 'Aller' : 'Entrée',
        action: () => onSubmit(value),
      });
    }

    // 2. Bangs correspondants
    const bangs = Object.entries(BANG_SHORTCUTS)
      .filter(([key, b]) => !q || key.startsWith(q) || b.label.toLowerCase().includes(q))
      .slice(0, q ? 3 : 4);
    bangs.forEach(([key, b]) => {
      out.push({
        id: `bang-${key}`,
        icon: <span className="text-sm leading-none">{b.emoji}</span>,
        title: b.label,
        subtitle: `Bang ${key} — rechercher sur ${b.label}`,
        badge: key,
        action: () => {
          if (!value.trim()) {
            onChange(`${key} `);
            inputRef.current?.focus();
          } else if (parsed.kind === 'bang') {
            onSubmit(value);
          } else {
            onSubmit(`${key} ${value.trim()}`);
          }
        },
      });
    });

    // 3. Onglets ouverts correspondants
    if (q) {
      tabs
        .filter((t) => t.url !== 'orakle://newtab')
        .sort((a, b) => fuzzyScore(q, b.title + b.url) - fuzzyScore(q, a.title + a.url))
        .slice(0, 3)
        .filter((t) => fuzzyScore(q, t.title + t.url) > 0)
        .forEach((t) => {
          out.push({
            id: `tab-${t.id}`,
            icon: <ScanLine className="w-4 h-4 text-emerald-400" />,
            title: t.title,
            subtitle: prettyUrl(t.url, 48),
            badge: 'Onglet ouvert',
            action: () => onSubmit(t.url),
          });
        });
    }

    // 4. Favoris
    if (q) {
      bookmarks
        .map((b) => ({ b, s: fuzzyScore(q, b.title + b.url) }))
        .filter((x) => x.s > 0)
        .sort((a, b) => b.s - a.s)
        .slice(0, 3)
        .forEach(({ b }) => {
          out.push({
            id: `bm-${b.id}`,
            icon: <Star className="w-4 h-4 text-amber-400" />,
            title: b.title,
            subtitle: prettyUrl(b.url, 44),
            badge: b.category,
            action: () => onSubmit(b.url),
          });
        });

      // 5. Historique
      history
        .map((h) => ({ h, s: fuzzyScore(q, h.title + h.url) }))
        .filter((x) => x.s > 0)
        .sort((a, b) => b.s - a.s)
        .slice(0, 3)
        .forEach(({ h }) => {
          out.push({
            id: `hist-${h.id}`,
            icon: <History className="w-4 h-4 text-[var(--text-3)]" />,
            title: h.title,
            subtitle: prettyUrl(h.url, 44),
            badge: 'Historique',
            action: () => onSubmit(h.url),
          });
        });
    }

    // 6. Actions rapides permanentes
    out.push(
      {
        id: 'act-shield',
        icon: <ShieldCheck className="w-4 h-4 text-emerald-400" />,
        title: 'Orakle Shield — inspecter la sécurité',
        subtitle: `${shieldBlocked} éléments bloqués sur cette session`,
        badge: 'Action',
        action: onOpenShield,
      },
      {
        id: 'act-palette',
        icon: <Command className="w-4 h-4 text-[var(--accent)]" />,
        title: 'Palette de commandes',
        subtitle: 'Toutes les actions Orakle, au clavier',
        badge: 'Ctrl K',
        action: onOpenPalette,
      }
    );

    return out;
  }, [
    value,
    parsed,
    bookmarks,
    history,
    tabs,
    onSubmit,
    onChange,
    onOpenShield,
    onOpenPalette,
    shieldBlocked,
    inputRef,
  ]);

  useEffect(() => setHighlight(0), [value]);

  // Fermeture au clic extérieur
  useEffect(() => {
    if (!focused) return;
    const onDown = (e: MouseEvent) => {
      if (wrapRef.current && !wrapRef.current.contains(e.target as Node)) {
        onFocusChange(false);
      }
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [focused, onFocusChange]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlight((h) => Math.min(suggestions.length - 1, h + 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlight((h) => Math.max(0, h - 1));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const target = suggestions[highlight];
      if (target) target.action();
      else onSubmit(value);
      onFocusChange(false);
      inputRef.current?.blur();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onFocusChange(false);
      inputRef.current?.blur();
    }
  };

  const securityIcon =
    securityState === 'secure' ? (
      <Lock className="w-3.5 h-3.5 text-emerald-400" />
    ) : securityState === 'internal' ? (
      <Sparkles className="w-3.5 h-3.5 text-[var(--accent)]" />
    ) : (
      <Unlock className="w-3.5 h-3.5 text-amber-400" />
    );

  return (
    <div ref={wrapRef} className="relative flex-1 max-w-[720px] mx-auto">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          onSubmit(value);
          onFocusChange(false);
          inputRef.current?.blur();
        }}
        className="omnibox-shell app-no-drag"
        data-focused={focused}
      >
        <button
          type="button"
          onClick={onOpenShield}
          className="flex items-center gap-1.5 text-[11px] font-semibold px-1.5 py-1 rounded-full hover:bg-[var(--fill-strong)] transition-colors cursor-pointer shrink-0"
          title="Sécurité de la connexion & Orakle Shield"
        >
          {securityIcon}
          {shieldEnabled && (
            <span className="hidden md:inline tabular-nums text-[10px] text-[var(--text-3)]">
              {shieldBlocked}
            </span>
          )}
        </button>

        <input
          ref={inputRef}
          type="text"
          value={value}
          onFocus={(e) => {
            onFocusChange(true);
            e.target.select();
          }}
          onKeyDown={onKeyDown}
          onChange={(e) => {
            onChange(e.target.value);
            onFocusChange(true);
          }}
          placeholder={`Rechercher avec ${SEARCH_ENGINES[searchEngine].name} · saisir une URL · !bang`}
          className="w-full bg-transparent text-[13px] text-[var(--text)] placeholder-[var(--text-3)] focus:outline-none focus-visible:outline-none tracking-[-0.01em]"
          spellCheck={false}
          autoComplete="off"
        />

        {zoomFactor !== 1 && (
          <button
            type="button"
            onClick={onResetZoom}
            className="px-1.5 py-0.5 rounded-md text-[10px] font-bold bg-[var(--fill-strong)] text-[var(--text-2)] hover:text-[var(--text)] cursor-pointer shrink-0"
            title="Réinitialiser le zoom à 100 %"
          >
            {Math.round(zoomFactor * 100)}%
          </button>
        )}

        <button
          type="button"
          onClick={onToggleBookmark}
          className="p-1 rounded-full hover:bg-[var(--fill-strong)] transition-colors cursor-pointer shrink-0"
          title={isBookmarked ? 'Retirer des favoris' : 'Ajouter aux favoris'}
        >
          <Star
            className={`w-3.5 h-3.5 transition-transform hover:scale-110 ${
              isBookmarked ? 'fill-amber-400 text-amber-400' : 'text-[var(--text-3)]'
            }`}
          />
        </button>
      </form>

      {/* Panneau de suggestions */}
      {focused && (
        <div className="absolute left-0 right-0 top-[calc(100%+8px)] z-50 glass-panel-strong rounded-[22px] overflow-hidden animate-pop">
          <div className="flex items-center justify-between px-3.5 pt-2.5 pb-1.5">
            <span className="text-[10px] font-bold uppercase tracking-[0.12em] text-[var(--text-3)]">
              {value.trim() ? 'Suggestions Orakle' : 'Accès Rapides'}
            </span>
            <span className="text-[10px] text-[var(--text-3)] flex items-center gap-1">
              <kbd className="kbd">↑</kbd>
              <kbd className="kbd">↓</kbd>
              <kbd className="kbd">↵</kbd>
            </span>
          </div>
          <div className="max-h-[380px] overflow-y-auto pb-1.5">
            {suggestions.map((s, i) => (
              <button
                key={s.id}
                type="button"
                onMouseEnter={() => setHighlight(i)}
                onClick={() => {
                  s.action();
                  onFocusChange(false);
                  inputRef.current?.blur();
                }}
                className={`w-[calc(100%-12px)] mx-1.5 flex items-center gap-3 px-2.5 py-2 rounded-xl text-left transition-colors cursor-pointer ${
                  i === highlight ? 'bg-[var(--accent-soft)]' : 'hover:bg-[var(--fill)]'
                }`}
              >
                <span className="w-7 h-7 rounded-lg bg-[var(--fill)] border border-[var(--stroke)] flex items-center justify-center shrink-0">
                  {s.icon}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-[12.5px] font-semibold text-[var(--text)] truncate">
                    {s.title}
                  </span>
                  {s.subtitle && (
                    <span className="block text-[10.5px] text-[var(--text-3)] truncate">
                      {s.subtitle}
                    </span>
                  )}
                </span>
                {s.badge && (
                  <span className="chip shrink-0 !text-[9.5px] uppercase tracking-wide">
                    {s.badge}
                  </span>
                )}
                {i === highlight && <CornerDownLeft className="w-3.5 h-3.5 text-[var(--accent)] shrink-0" />}
              </button>
            ))}
          </div>
          <div className="flex items-center justify-between px-3.5 py-1.5 border-t border-[var(--stroke)] bg-[var(--fill)]">
            <span className="text-[10px] text-[var(--text-3)] flex items-center gap-1.5">
              <ShieldAlert className="w-3 h-3 text-emerald-400" />
              Requêtes purgées des paramètres de pistage
            </span>
            <span className="text-[10px] text-[var(--text-3)] flex items-center gap-1">
              <Gauge className="w-3 h-3" />
              Orakle Aurora
            </span>
          </div>
        </div>
      )}
    </div>
  );
};
