import React, { useEffect, useMemo, useState } from 'react';
import { Play, Pause, RotateCcw, X, Brain, ShieldOff, ShieldCheck, Timer } from 'lucide-react';

interface FocusModeProps {
  isOpen: boolean;
  onClose: () => void;
  blockList: string[];
  currentHost: string;
}

/**
 * Orakle Focus — minuteur Pomodoro + rappel de blocage des domaines distrayants.
 * Le contournement (Blocklist) est appliqué côté App sur la navigation.
 */
export const FocusMode: React.FC<FocusModeProps> = ({ isOpen, onClose, blockList, currentHost }) => {
  const [duration, setDuration] = useState(25);
  const [remaining, setRemaining] = useState(25 * 60);
  const [running, setRunning] = useState(false);
  const [cycles, setCycles] = useState(0);

  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => {
      setRemaining((r) => {
        if (r <= 1) {
          setRunning(false);
          setCycles((c) => c + 1);
          return 0;
        }
        return r - 1;
      });
    }, 1000);
    return () => clearInterval(id);
  }, [running]);

  useEffect(() => {
    if (!running) setRemaining(duration * 60);
  }, [duration, running]);

  const progress = useMemo(
    () => 1 - remaining / Math.max(1, duration * 60),
    [remaining, duration]
  );

  if (!isOpen) return null;

  const mm = String(Math.floor(remaining / 60)).padStart(2, '0');
  const ss = String(remaining % 60).padStart(2, '0');
  const blockedNow = blockList.some((b) => currentHost.includes(b.replace(/^www\./, '')));

  const R = 78;
  const C = 2 * Math.PI * R;

  return (
    <div className="fixed inset-0 z-[55] animate-overlay flex items-center justify-center" style={{ background: 'rgba(4,5,10,0.78)' }}>
      <div className="glass-panel-strong rounded-[32px] p-8 w-[420px] max-w-[92vw] animate-pop">
        <button
          type="button"
          onClick={onClose}
          className="icon-btn w-8 h-8 !absolute top-4 right-4"
          aria-label="Fermer le mode Focus"
        >
          <X className="w-4 h-4" />
        </button>

        <div className="flex items-center gap-2 mb-6">
          <Brain className="w-4 h-4 text-[var(--accent)]" />
          <h2 className="text-sm font-bold tracking-tight">Mode Focus</h2>
          <span className="chip ml-auto !text-[9.5px]">
            <Timer className="w-2.5 h-2.5" />
            {cycles} cycle(s)
          </span>
        </div>

        <div className="relative mx-auto w-[190px] h-[190px] mb-6">
          <svg viewBox="0 0 190 190" className="w-full h-full -rotate-90">
            <circle cx="95" cy="95" r={R} fill="none" stroke="var(--fill-strong)" strokeWidth="9" />
            <circle
              cx="95"
              cy="95"
              r={R}
              fill="none"
              stroke="url(#focusGrad)"
              strokeWidth="9"
              strokeLinecap="round"
              strokeDasharray={C}
              strokeDashoffset={C * (1 - progress)}
              style={{ transition: 'stroke-dashoffset 1s linear' }}
            />
            <defs>
              <linearGradient id="focusGrad" x1="0" y1="0" x2="1" y2="1">
                <stop offset="0%" stopColor="var(--accent)" />
                <stop offset="100%" stopColor="var(--accent-2)" />
              </linearGradient>
            </defs>
          </svg>
          <div className="absolute inset-0 flex flex-col items-center justify-center">
            <span className="text-4xl font-extrabold tabular-nums tracking-tight">
              {mm}:{ss}
            </span>
            <span className="text-[10px] uppercase tracking-[0.18em] text-[var(--text-3)] mt-1">
              {running ? 'En cours' : 'Prêt'}
            </span>
          </div>
        </div>

        <div className="flex items-center justify-center gap-2 mb-5">
          <button
            type="button"
            onClick={() => setRunning((r) => !r)}
            className="btn-primary !px-5 !py-2 !text-xs"
          >
            {running ? <Pause className="w-3.5 h-3.5" /> : <Play className="w-3.5 h-3.5" />}
            {running ? 'Pause' : 'Démarrer'}
          </button>
          <button
            type="button"
            onClick={() => {
              setRunning(false);
              setRemaining(duration * 60);
            }}
            className="btn-glass !py-2"
          >
            <RotateCcw className="w-3.5 h-3.5" />
            Réinitialiser
          </button>
        </div>

        <div className="segmented w-full justify-center mb-5">
          {[15, 25, 45, 60].map((d) => (
            <button
              key={d}
              type="button"
              data-active={duration === d}
              onClick={() => {
                setDuration(d);
                setRemaining(d * 60);
                setRunning(false);
              }}
            >
              {d} min
            </button>
          ))}
        </div>

        <div className="rounded-2xl bg-[var(--fill)] border border-[var(--stroke)] p-3 space-y-2">
          <div className="flex items-center gap-2 text-[11px] font-bold">
            {blockedNow ? (
              <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
            ) : (
              <ShieldOff className="w-3.5 h-3.5 text-[var(--text-3)]" />
            )}
            Sites distrayants bloqués pendant le Focus
          </div>
          <div className="flex flex-wrap gap-1">
            {blockList.length === 0 && (
              <span className="text-[10px] text-[var(--text-3)]">
                Liste vide — configurez-la dans Paramètres › Focus.
              </span>
            )}
            {blockList.map((b) => (
              <span
                key={b}
                className="chip !text-[9.5px]"
                style={
                  currentHost.includes(b.replace(/^www\./, ''))
                    ? {
                        color: 'var(--danger)',
                        borderColor: 'color-mix(in srgb, var(--danger) 35%, transparent)',
                        background: 'color-mix(in srgb, var(--danger) 12%, transparent)',
                      }
                    : undefined
                }
              >
                {b}
              </span>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};
