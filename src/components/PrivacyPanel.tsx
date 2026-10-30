import React, { useEffect, useState } from 'react';
import {
  Cookie,
  Trash2,
  RefreshCw,
  Loader2,
  ShieldCheck,
  ShieldAlert,
  Lock,
  Globe2,
  Eye,
  EyeOff,
} from 'lucide-react';
import { CookieInfo } from '../types/browser';
import { formatBytes, relativeTime } from '../utils/url';

/**
 * Inspecteur de cookies & traces — vision « cybersécurité » des données
 * déposées par le site courant, avec purge sélective par domaine.
 */
export const PrivacyPanel: React.FC<{ currentHost: string }> = ({ currentHost }) => {
  const [cookies, setCookies] = useState<CookieInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [reveal, setReveal] = useState(false);
  const [filter, setFilter] = useState<'all' | 'tracker' | 'session'>('all');

  const load = async () => {
    if (!window.orakle) return;
    setLoading(true);
    try {
      const list = await window.orakle.privacy.cookies();
      setCookies(list);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const scoped = cookies.filter((c) =>
    currentHost ? c.domain.replace(/^\./, '').includes(currentHost) : true
  );
  const shown = scoped.filter((c) =>
    filter === 'all' ? true : filter === 'tracker' ? c.risk === 'tracker' : c.session
  );

  const trackers = scoped.filter((c) => c.risk === 'tracker');
  const totalBytes = scoped.reduce((acc, c) => acc + c.size, 0);

  const purgeDomain = async (domain: string) => {
    await window.orakle?.privacy.clearCookies(domain.replace(/^\./, ''));
    load();
  };

  return (
    <div className="space-y-3">
      <div className="orakle-card p-3.5 space-y-3">
        <div className="flex items-center gap-2">
          <Cookie className="w-4 h-4 text-[var(--accent)]" />
          <span className="text-xs font-bold text-[var(--text)]">Cookies & Stockage</span>
          <button
            type="button"
            onClick={load}
            className="icon-btn w-6 h-6 ml-auto"
            title="Rafraîchir"
          >
            {loading ? <Loader2 className="w-3 h-3 animate-spin" /> : <RefreshCw className="w-3 h-3" />}
          </button>
          <button
            type="button"
            onClick={() => setReveal((r) => !r)}
            className="icon-btn w-6 h-6"
            title={reveal ? 'Masquer les valeurs' : 'Afficher les valeurs'}
          >
            {reveal ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
          </button>
        </div>

        <div className="grid grid-cols-3 gap-1.5">
          <div className="rounded-xl bg-[var(--fill)] border border-[var(--stroke)] p-2 text-center">
            <div className="text-base font-extrabold tabular-nums text-[var(--text)]">{scoped.length}</div>
            <div className="text-[9px] uppercase tracking-wide text-[var(--text-3)]">Cookies</div>
          </div>
          <div className="rounded-xl bg-[var(--fill)] border border-[var(--stroke)] p-2 text-center">
            <div className="text-base font-extrabold tabular-nums text-amber-400">{trackers.length}</div>
            <div className="text-[9px] uppercase tracking-wide text-[var(--text-3)]">Traqueurs</div>
          </div>
          <div className="rounded-xl bg-[var(--fill)] border border-[var(--stroke)] p-2 text-center">
            <div className="text-base font-extrabold tabular-nums text-[var(--accent)]">
              {formatBytes(totalBytes)}
            </div>
            <div className="text-[9px] uppercase tracking-wide text-[var(--text-3)]">Empreinte</div>
          </div>
        </div>

        <div className="segmented w-full justify-center">
          {(
            [
              ['all', 'Tous'],
              ['tracker', 'Traqueurs'],
              ['session', 'Session'],
            ] as const
          ).map(([id, label]) => (
            <button key={id} type="button" data-active={filter === id} onClick={() => setFilter(id)}>
              {label}
            </button>
          ))}
        </div>
      </div>

      <div className="space-y-1.5">
        {shown.length === 0 && !loading && (
          <div className="orakle-card p-6 text-center text-[11px] text-[var(--text-3)]">
            Aucun cookie pour ce filtre.
          </div>
        )}
        {shown.slice(0, 80).map((c) => (
          <div
            key={`${c.domain}-${c.name}`}
            className="orakle-card !rounded-xl p-2.5 space-y-1"
          >
            <div className="flex items-center gap-1.5">
              <span
                className="w-1.5 h-1.5 rounded-full shrink-0"
                style={{
                  background:
                    c.risk === 'tracker'
                      ? 'var(--danger)'
                      : c.session
                      ? 'var(--warn)'
                      : 'var(--ok)',
                }}
              />
              <span className="text-[11px] font-bold text-[var(--text)] truncate">{c.name}</span>
              <span className="text-[9px] text-[var(--text-3)] ml-auto shrink-0 flex items-center gap-1">
                {c.secure ? <Lock className="w-2.5 h-2.5 text-emerald-400" /> : <Globe2 className="w-2.5 h-2.5" />}
                {formatBytes(c.size)}
              </span>
            </div>
            <div className="text-[9.5px] text-[var(--text-3)] truncate font-mono">
              {reveal ? c.value.slice(0, 90) || '(vide)' : '••••••••••••'}
            </div>
            <div className="flex items-center gap-1.5 text-[9px] text-[var(--text-3)]">
              <span className="truncate">{c.domain}</span>
              <span className="ml-auto shrink-0 flex items-center gap-1.5">
                {c.httpOnly && <span className="chip !text-[8px] !py-0">HttpOnly</span>}
                <span className="chip !text-[8px] !py-0">{c.sameSite}</span>
                <span className="shrink-0">
                  {c.session ? 'session' : c.expires ? relativeTime(c.expires) : '—'}
                </span>
                <button
                  type="button"
                  onClick={() => purgeDomain(c.domain)}
                  className="text-[var(--text-3)] hover:text-red-400 cursor-pointer"
                  title="Supprimer les cookies de ce domaine"
                >
                  <Trash2 className="w-2.5 h-2.5" />
                </button>
              </span>
            </div>
          </div>
        ))}
      </div>

      {scoped.length > 0 && (
        <button
          type="button"
          onClick={async () => {
            for (const domain of Array.from(new Set(scoped.map((c) => c.domain.replace(/^\./, ''))))) {
              await window.orakle?.privacy.clearCookies(domain);
            }
            load();
          }}
          className="w-full py-2 rounded-xl text-[11px] font-bold bg-red-500/12 hover:bg-red-500/20 text-red-400 border border-red-500/25 transition-colors cursor-pointer flex items-center justify-center gap-1.5"
        >
          <Trash2 className="w-3 h-3" />
          Purger les {scoped.length} cookies affichés
        </button>
      )}

      <div className="orakle-card p-3 flex gap-2">
        {trackers.length > 0 ? (
          <ShieldAlert className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
        ) : (
          <ShieldCheck className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
        )}
        <p className="text-[10.5px] leading-relaxed text-[var(--text-2)]">
          {trackers.length > 0
            ? `${trackers.length} cookie(s) portent une signature de régie publicitaire connue. Orakle Shield les bloque déjà au niveau réseau ; la purge locale supprime les traces déjà déposées.`
            : 'Aucun cookie de pistage connu détecté pour ce domaine. Les cookies de session disparaissent à la fermeture du navigateur.'}
        </p>
      </div>
    </div>
  );
};
