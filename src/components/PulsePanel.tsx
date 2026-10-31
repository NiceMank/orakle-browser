import React, { useState } from 'react';
import {
  Gauge,
  Activity,
  ArrowDownToLine,
  Server,
  ShieldCheck,
  RefreshCw,
  Loader2,
  Clock,
  Info,
} from 'lucide-react';
import { NetworkTimingResult } from '../types/browser';
import { formatBytes, formatLatency, prettyUrl } from '../utils/url';

interface PulsePanelProps {
  currentUrl: string;
}

const PHASES = [
  { key: 'dnsMs', label: 'DNS', color: '#38bdf8' },
  { key: 'connectMs', label: 'TCP', color: '#8b5cf6' },
  { key: 'tlsMs', label: 'TLS', color: '#10b981' },
  { key: 'firstByteMs', label: 'TTFB', color: '#f59e0b' },
] as const;

function speedGrade(ms: number) {
  if (ms <= 0) return { grade: '—', color: 'var(--text-3)', label: 'Non mesuré' };
  if (ms < 250) return { grade: 'A+', color: '#30d158', label: 'Éclair' };
  if (ms < 600) return { grade: 'A', color: '#30d158', label: 'Très rapide' };
  if (ms < 1200) return { grade: 'B', color: '#38bdf8', label: 'Rapide' };
  if (ms < 2500) return { grade: 'C', color: '#ff9f0a', label: 'Correct' };
  return { grade: 'D', color: '#ff453a', label: 'Lent' };
}

/**
 * Orakle Pulse — diagnostic de latence réseau en direct sur l'onglet actif.
 */
export const PulsePanel: React.FC<PulsePanelProps> = ({ currentUrl }) => {
  const [result, setResult] = useState<NetworkTimingResult | null>(null);
  const [loading, setLoading] = useState(false);

  const analyse = async () => {
    if (!currentUrl || currentUrl.startsWith('orakle://') || !window.orakle) return;
    setLoading(true);
    try {
      const res = await window.orakle.network.probe(currentUrl);
      setResult(res);
    } finally {
      setLoading(false);
    }
  };

  const grade = speedGrade(result?.firstByteMs || 0);
  const max = Math.max(1, ...PHASES.map((p) => (result ? Number(result[p.key]) || 0 : 0)));

  return (
    <div className="space-y-3">
      <div className="orakle-card p-3.5 space-y-3">
        <div className="flex items-center gap-2">
          <Gauge className="w-4 h-4 text-[var(--accent)]" />
          <span className="text-xs font-bold text-[var(--text)]">Latence réseau</span>
          <button
            type="button"
            onClick={analyse}
            disabled={loading || !currentUrl || currentUrl.startsWith('orakle://')}
            className="btn-glass ml-auto !py-1 !px-2 !text-[10.5px] disabled:opacity-40"
          >
            {loading ? <Loader2 className="w-3 h-3 animate-spin" /> : <RefreshCw className="w-3 h-3" />}
            Mesurer
          </button>
        </div>

        <div className="text-[10.5px] text-[var(--text-3)] truncate">
          {prettyUrl(currentUrl, 52) || 'Aucune page active'}
        </div>

        <div className="flex items-center gap-3">
          <div
            className="w-16 h-16 rounded-2xl flex flex-col items-center justify-center shrink-0 border"
            style={{
              borderColor: `color-mix(in srgb, ${grade.color} 32%, transparent)`,
              background: `color-mix(in srgb, ${grade.color} 12%, transparent)`,
            }}
          >
            <span className="text-xl font-extrabold tabular-nums" style={{ color: grade.color }}>
              {grade.grade}
            </span>
            <span className="text-[8.5px] uppercase tracking-wider text-[var(--text-3)]">
              {grade.label}
            </span>
          </div>
          <div className="flex-1 space-y-1.5">
            {PHASES.map((p) => {
              const value = result ? Number(result[p.key]) || 0 : 0;
              return (
                <div key={p.key} className="flex items-center gap-2">
                  <span className="text-[9.5px] font-bold w-8 text-[var(--text-3)]">{p.label}</span>
                  <div className="flex-1 h-1.5 rounded-full bg-[var(--fill-strong)] overflow-hidden">
                    <div
                      className="h-full rounded-full transition-all duration-700"
                      style={{
                        width: `${Math.min(100, (value / max) * 100)}%`,
                        backgroundColor: p.color,
                      }}
                    />
                  </div>
                  <span className="text-[9.5px] tabular-nums text-[var(--text-2)] w-12 text-right">
                    {formatLatency(value)}
                  </span>
                </div>
              );
            })}
          </div>
        </div>

        {result && (
          <div className="grid grid-cols-2 gap-1.5 pt-1">
            <Metric icon={<Clock className="w-3 h-3" />} label="Total" value={formatLatency(result.totalMs)} />
            <Metric icon={<Activity className="w-3 h-3" />} label="Statut" value={result.ok ? `HTTP ${result.statusCode}` : result.error || 'Échec'} />
            <Metric icon={<Server className="w-3 h-3" />} label="Serveur" value={result.server} />
            <Metric icon={<ArrowDownToLine className="w-3 h-3" />} label="Poids" value={result.contentLength ? formatBytes(result.contentLength) : 'non déclaré'} />
            <Metric icon={<ShieldCheck className="w-3 h-3" />} label="Protocole" value={result.protocol || '—'} />
            <Metric icon={<Info className="w-3 h-3" />} label="Cache CDN" value={result.cached ? 'HIT' : 'MISS'} />
          </div>
        )}

        {!result && !loading && (
          <p className="text-[10.5px] leading-relaxed text-[var(--text-3)] flex gap-1.5">
            <Info className="w-3 h-3 shrink-0 mt-0.5" />
            La sonde effectue une requête réelle et mesure les phases DNS, TCP, TLS et le premier
            octet (TTFB) — utile pour diagnostiquer un site lent ou un CDN mal réglé.
          </p>
        )}
      </div>
    </div>
  );
};

const Metric: React.FC<{ icon: React.ReactNode; label: string; value: string }> = ({
  icon,
  label,
  value,
}) => (
  <div className="rounded-lg bg-[var(--fill)] border border-[var(--stroke)] px-2 py-1.5">
    <div className="flex items-center gap-1 text-[9px] uppercase tracking-wide text-[var(--text-3)]">
      {icon}
      {label}
    </div>
    <div className="text-[11px] font-semibold text-[var(--text)] truncate tabular-nums">{value}</div>
  </div>
);
