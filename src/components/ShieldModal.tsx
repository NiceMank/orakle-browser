import React, { useState, useEffect } from 'react';
import {
  Shield,
  ShieldCheck,
  ShieldAlert,
  X,
  CheckCircle2,
  AlertTriangle,
  RefreshCw,
  Lock,
  Unlock,
  Activity,
  UserCheck,
  Sparkles,
} from 'lucide-react';
import { ShieldStats, SecurityAuditResult, BrowserSettings } from '../types/browser';
import { extractHostname, formatBytes } from '../utils/url';

interface ShieldModalProps {
  isOpen: boolean;
  onClose: () => void;
  currentUrl: string;
  shieldStats: ShieldStats;
  settings: BrowserSettings;
  onUpdateSettings: (patch: Partial<BrowserSettings>) => void;
  onToggleHostWhitelist: (host: string) => void;
}

const UA_OPTIONS = [
  { id: 'standard', label: 'Orakle Standard (Linux x64)' },
  { id: 'chrome-win', label: 'Chrome 131 — Windows 11' },
  { id: 'firefox-linux', label: 'Firefox 133 — Linux x86_64' },
  { id: 'safari-mac', label: 'Safari 18.1 — macOS Sequoia' },
  { id: 'mobile-ios', label: 'iPhone 16 Pro — iOS 18' },
];

export const ShieldModal: React.FC<ShieldModalProps> = ({
  isOpen,
  onClose,
  currentUrl,
  shieldStats,
  settings,
  onUpdateSettings,
  onToggleHostWhitelist,
}) => {
  const [activeSubTab, setActiveSubTab] = useState<'shield' | 'headers' | 'log'>('shield');
  const [audit, setAudit] = useState<SecurityAuditResult | null>(null);
  const [auditing, setAuditing] = useState(false);

  const currentHost = extractHostname(currentUrl);
  const isInternal = !currentUrl || currentUrl.startsWith('orakle://');
  const isWhitelisted = shieldStats.whitelistedHosts.includes(currentHost.toLowerCase());

  const runSecurityAudit = async () => {
    if (isInternal || !window.orakle) {
      setAudit(null);
      return;
    }
    setAuditing(true);
    try {
      const res = await window.orakle.security.auditUrl(currentUrl);
      setAudit(res);
    } finally {
      setAuditing(false);
    }
  };

  useEffect(() => {
    if (isOpen && !isInternal) {
      runSecurityAudit();
    } else if (isInternal) {
      setAudit(null);
    }
  }, [isOpen, currentUrl]);

  if (!isOpen) return null;

  const gradeColor = (grade?: SecurityAuditResult['grade']) => {
    switch (grade) {
      case 'A+':
      case 'A':
        return 'text-emerald-400 bg-emerald-500/15 border-emerald-500/30';
      case 'B':
        return 'text-blue-400 bg-blue-500/15 border-blue-500/30';
      case 'C':
        return 'text-amber-400 bg-amber-500/15 border-amber-500/30';
      default:
        return 'text-red-400 bg-red-500/15 border-red-500/30';
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-end p-4 pt-20 bg-black/40 backdrop-blur-xs"
      onClick={onClose}
    >
      <div
        className="glass-panel-strong w-full max-w-md rounded-3xl overflow-hidden flex flex-col max-h-[82vh] animate-in fade-in slide-in-from-top-3 duration-200"
        onClick={(e) => e.stopPropagation()}
      >
        {/* En-tête du Bouclier */}
        <div className="p-4 border-b border-[var(--stroke)] flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div
              className={`w-10 h-10 rounded-2xl flex items-center justify-center border ${
                shieldStats.enabled && !isWhitelisted
                  ? 'bg-emerald-500/15 text-emerald-400 border-emerald-500/30'
                  : 'bg-amber-500/15 text-amber-400 border-amber-500/30'
              }`}
            >
              {shieldStats.enabled && !isWhitelisted ? (
                <ShieldCheck className="w-5 h-5" />
              ) : (
                <ShieldAlert className="w-5 h-5" />
              )}
            </div>
            <div>
              <h3 className="text-sm font-bold text-[var(--text)] flex items-center gap-2">
                Orakle Shield & Cyber Audit
              </h3>
              <p className="text-xs text-[var(--text-2)] truncate max-w-[240px]">
                {isInternal ? 'Page système sécurisée Orakle' : currentHost}
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={onClose}
            className="icon-btn w-8 h-8"
            aria-label="Fermer le panneau Orakle Shield"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Navigation par onglets internes */}
        <div className="grid grid-cols-3 gap-1 p-2 bg-[var(--fill)] border-b border-[var(--stroke)] text-xs font-semibold">
          <button
            type="button"
            onClick={() => setActiveSubTab('shield')}
            className={`py-1.5 rounded-xl cursor-pointer transition-colors ${
              activeSubTab === 'shield'
                ? 'bg-[var(--bg-elev)] text-[var(--text)] shadow-xs'
                : 'text-[var(--text-2)] hover:text-[var(--text)]'
            }`}
          >
            Protection
          </button>
          <button
            type="button"
            onClick={() => setActiveSubTab('headers')}
            className={`py-1.5 rounded-xl cursor-pointer transition-colors flex items-center justify-center gap-1.5 ${
              activeSubTab === 'headers'
                ? 'bg-[var(--bg-elev)] text-[var(--text)] shadow-xs'
                : 'text-[var(--text-2)] hover:text-[var(--text)]'
            }`}
          >
            Audit OWASP
            {audit && (
              <span className="px-1.5 py-0.2 rounded-md text-[10px] font-extrabold bg-[var(--accent-soft)] text-[var(--accent)]">
                {audit.grade}
              </span>
            )}
          </button>
          <button
            type="button"
            onClick={() => setActiveSubTab('log')}
            className={`py-1.5 rounded-xl cursor-pointer transition-colors ${
              activeSubTab === 'log'
                ? 'bg-[var(--bg-elev)] text-[var(--text)] shadow-xs'
                : 'text-[var(--text-2)] hover:text-[var(--text)]'
            }`}
          >
            Journal ({shieldStats.recentBlocked.length})
          </button>
        </div>

        {/* Contenu de l'onglet actif */}
        <div className="p-4 overflow-y-auto space-y-4 flex-1">
          {activeSubTab === 'shield' && (
            <>
              {/* Compteurs rapides */}
              <div className="grid grid-cols-3 gap-2.5 text-center">
                <div className="orakle-card p-3">
                  <div className="text-xl font-extrabold text-emerald-400 tabular-nums">
                    {shieldStats.totalBlocked}
                  </div>
                  <div className="text-[11px] text-[var(--text-2)]">Bloqués</div>
                </div>
                <div className="orakle-card p-3">
                  <div className="text-xl font-extrabold text-blue-400 tabular-nums">
                    {shieldStats.paramsStripped}
                  </div>
                  <div className="text-[11px] text-[var(--text-2)]">UTM purgés</div>
                </div>
                <div className="orakle-card p-3">
                  <div className="text-xl font-extrabold text-amber-400 tabular-nums">
                    {formatBytes(shieldStats.estimatedBytesSaved)}
                  </div>
                  <div className="text-[11px] text-[var(--text-2)]">Économisés</div>
                </div>
              </div>

              {/* Contrôles du bouclier */}
              <div className="space-y-2.5">
                <label className="orakle-card p-3.5 flex items-center justify-between cursor-pointer">
                  <div className="flex items-center gap-3">
                    <Shield className="w-4 h-4 text-[var(--accent)]" />
                    <div>
                      <div className="text-xs font-semibold text-[var(--text)]">
                        Bouclier Anti-Pubs & Traqueurs
                      </div>
                      <div className="text-[11px] text-[var(--text-2)]">
                        Bloque les régies publicitaires et scripts de pistage
                      </div>
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.shieldEnabled}
                    onChange={(e) => onUpdateSettings({ shieldEnabled: e.target.checked })}
                    className="w-4 h-4 accent-[var(--accent)] cursor-pointer"
                  />
                </label>

                {!isInternal && (
                  <label className="orakle-card p-3.5 flex items-center justify-between cursor-pointer">
                    <div className="flex items-center gap-3">
                      {isWhitelisted ? (
                        <Unlock className="w-4 h-4 text-amber-400" />
                      ) : (
                        <Lock className="w-4 h-4 text-emerald-400" />
                      )}
                      <div>
                        <div className="text-xs font-semibold text-[var(--text)]">
                          Protection active sur {currentHost}
                        </div>
                        <div className="text-[11px] text-[var(--text-2)]">
                          {isWhitelisted
                            ? 'Ce domaine est actuellement en liste blanche'
                            : 'Filtrage strict activé pour ce domaine'}
                        </div>
                      </div>
                    </div>
                    <input
                      type="checkbox"
                      checked={!isWhitelisted}
                      onChange={() => onToggleHostWhitelist(currentHost)}
                      className="w-4 h-4 accent-[var(--accent)] cursor-pointer"
                    />
                  </label>
                )}

                <label className="orakle-card p-3.5 flex items-center justify-between cursor-pointer">
                  <div className="flex items-center gap-3">
                    <Sparkles className="w-4 h-4 text-blue-400" />
                    <div>
                      <div className="text-xs font-semibold text-[var(--text)]">
                        Nettoyage des paramètres d’URL
                      </div>
                      <div className="text-[11px] text-[var(--text-2)]">
                        Supprime automatiquement utm_*, fbclid, gclid, msclkid
                      </div>
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.stripTrackingParams}
                    onChange={(e) => onUpdateSettings({ stripTrackingParams: e.target.checked })}
                    className="w-4 h-4 accent-[var(--accent)] cursor-pointer"
                  />
                </label>

                {/* Spoofing User-Agent */}
                <div className="orakle-card p-3.5 space-y-2">
                  <div className="flex items-center gap-2 text-xs font-semibold text-[var(--text)]">
                    <UserCheck className="w-4 h-4 text-purple-400" />
                    Empreinte User-Agent (Spoofing)
                  </div>
                  <select
                    value={settings.userAgentProfile}
                    onChange={(e) => onUpdateSettings({ userAgentProfile: e.target.value })}
                    className="w-full px-3 py-2 rounded-xl bg-[var(--fill)] border border-[var(--stroke)] text-xs text-[var(--text)] focus:outline-none"
                  >
                    {UA_OPTIONS.map((opt) => (
                      <option key={opt.id} value={opt.id} className="bg-[#141620] text-white">
                        {opt.label}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
            </>
          )}

          {activeSubTab === 'headers' && (
            <div className="space-y-3">
              {isInternal ? (
                <div className="orakle-card p-6 text-center space-y-2">
                  <ShieldCheck className="w-8 h-8 text-emerald-400 mx-auto" />
                  <div className="text-sm font-bold text-[var(--text)]">
                    Page Interne Orakle Sécurisée
                  </div>
                  <p className="text-xs text-[var(--text-2)]">
                    Ouvrez un site web externe (HTTP/HTTPS) pour auditer ses en-têtes de sécurité OWASP en direct.
                  </p>
                </div>
              ) : auditing ? (
                <div className="orakle-card p-8 flex flex-col items-center justify-center gap-2 text-xs text-[var(--text-2)]">
                  <RefreshCw className="w-6 h-6 animate-spin text-[var(--accent)]" />
                  Analyse des en-têtes HTTP/TLS de {currentHost}...
                </div>
              ) : audit ? (
                <>
                  <div className="orakle-card p-3.5 flex items-center justify-between">
                    <div>
                      <div className="text-xs font-bold text-[var(--text)]">
                        Score de Sécurité HTTP
                      </div>
                      <div className="text-[11px] text-[var(--text-2)]">
                        {audit.score}/100 · Statut HTTP {audit.statusCode || 'N/A'}
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <button
                        type="button"
                        onClick={runSecurityAudit}
                        className="icon-btn w-8 h-8"
                        title="Relancer l'audit"
                      >
                        <RefreshCw className="w-3.5 h-3.5" />
                      </button>
                      <div
                        className={`px-3 py-1 rounded-xl border text-base font-extrabold ${gradeColor(
                          audit.grade
                        )}`}
                      >
                        {audit.grade}
                      </div>
                    </div>
                  </div>

                  <div className="space-y-2">
                    {audit.checks.map((chk) => (
                      <div key={chk.header} className="orakle-card p-3 space-y-1">
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-xs font-semibold text-[var(--text)] flex items-center gap-1.5">
                            {chk.present ? (
                              <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                            ) : (
                              <AlertTriangle className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                            )}
                            {chk.name}
                          </span>
                          <span
                            className={`text-[10px] font-bold px-2 py-0.5 rounded-full ${
                              chk.present
                                ? 'bg-emerald-500/15 text-emerald-400'
                                : 'bg-amber-500/15 text-amber-400'
                            }`}
                          >
                            {chk.present ? 'Présent' : 'Manquant'}
                          </span>
                        </div>
                        <div className="text-[10px] font-mono text-[var(--text-3)] truncate">
                          {chk.value}
                        </div>
                        <div className="text-[11px] text-[var(--text-2)]">{chk.description}</div>
                      </div>
                    ))}
                  </div>
                </>
              ) : null}
            </div>
          )}

          {activeSubTab === 'log' && (
            <div className="space-y-2">
              {shieldStats.recentBlocked.length === 0 ? (
                <div className="orakle-card p-6 text-center text-xs text-[var(--text-3)]">
                  <Activity className="w-6 h-6 mx-auto mb-2 opacity-40" />
                  Aucune requête publicitaire ou traqueur intercepté pour le moment.
                </div>
              ) : (
                shieldStats.recentBlocked.map((item) => (
                  <div
                    key={item.id}
                    className="orakle-card p-2.5 flex items-center justify-between gap-2"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="text-xs font-semibold text-[var(--text)] truncate">
                        {item.domain}
                      </div>
                      <div className="text-[10px] text-[var(--text-3)] truncate">{item.url}</div>
                    </div>
                    <span className="px-2 py-0.5 rounded-full text-[10px] font-bold bg-red-500/15 text-red-400 shrink-0">
                      {item.category}
                    </span>
                  </div>
                ))
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
