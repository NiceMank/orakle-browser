import React, { useEffect, useState } from 'react';
import {
  X,
  Minus,
  Plus,
  Copy,
  Check,
  Sparkles,
  Loader2,
  BookOpen,
  Clock,
  Wand2,
  ExternalLink,
} from 'lucide-react';
import { ReaderArticle, BrowserSettings } from '../types/browser';

interface ReaderViewProps {
  isOpen: boolean;
  url: string;
  settings: BrowserSettings;
  onUpdateSettings: (patch: Partial<BrowserSettings>) => void;
  onClose: () => void;
  onNavigate: (url: string) => void;
}

const THEMES: Record<
  BrowserSettings['readerTheme'],
  { bg: string; text: string; muted: string; label: string }
> = {
  paper: { bg: '#fbfaf7', text: '#1a1b1e', muted: '#6b7280', label: 'Papier' },
  sepia: { bg: '#f4ecd8', text: '#3a2f1f', muted: '#7a6a52', label: 'Sépia' },
  night: { bg: '#12141c', text: '#e8eaf2', muted: '#9aa1b5', label: 'Nuit' },
};

/**
 * Mode Lecture « Orakle Reader » — extraction d'article + résumé IA / local.
 */
export const ReaderView: React.FC<ReaderViewProps> = ({
  isOpen,
  url,
  settings,
  onUpdateSettings,
  onClose,
  onNavigate,
}) => {
  const [article, setArticle] = useState<ReaderArticle | null>(null);
  const [loading, setLoading] = useState(false);
  const [summary, setSummary] = useState<{ text: string; engine: string } | null>(null);
  const [summarizing, setSummarizing] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!isOpen || !url || !window.orakle) return;
    let cancelled = false;
    setLoading(true);
    setArticle(null);
    setSummary(null);

    window.orakle.reader
      .extract(url)
      .then((res) => {
        if (cancelled) return;
        setArticle(res);
        if (res && settings.aiAutoSummarize) {
          setSummarizing(true);
          window.orakle?.reader
            .summarize(res.textContent)
            .then((s) => !cancelled && setSummary({ text: s.summary, engine: s.engine }))
            .finally(() => !cancelled && setSummarizing(false));
        }
      })
      .finally(() => !cancelled && setLoading(false));

    return () => {
      cancelled = true;
    };
  }, [isOpen, url, settings.aiAutoSummarize]);

  const runSummary = async () => {
    if (!article) return;
    setSummarizing(true);
    try {
      const res = await window.orakle?.reader.summarize(article.textContent);
      if (res) setSummary({ text: res.summary, engine: res.engine });
    } finally {
      setSummarizing(false);
    }
  };

  if (!isOpen) return null;

  const theme = THEMES[settings.readerTheme];

  const copyArticle = async () => {
    if (!article) return;
    try {
      await navigator.clipboard.writeText(`${article.title}\n${article.url}\n\n${article.textContent}`);
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    } catch {
      /* noop */
    }
  };

  return (
    <div className="fixed inset-0 z-[60] animate-overlay" style={{ background: 'rgba(3,4,8,0.72)' }}>
      <div
        className="absolute inset-0 overflow-y-auto"
        style={{ backgroundColor: theme.bg, color: theme.text }}
      >
        {/* Barre d'outils flottante */}
        <div className="sticky top-0 z-10 glass-panel-strong !bg-[var(--bg-glass-strong)] border-b border-[var(--stroke)]">
          <div className="max-w-[760px] mx-auto px-5 py-2.5 flex items-center justify-between gap-3">
            <div className="flex items-center gap-2 min-w-0">
              <BookOpen className="w-4 h-4 text-[var(--accent)] shrink-0" />
              <span className="text-[11px] font-bold uppercase tracking-[0.14em] text-[var(--text-2)]">
                Mode Lecture
              </span>
              {article && (
                <>
                  <span className="chip !text-[9.5px]">
                    <Clock className="w-2.5 h-2.5" />
                    {article.readingMinutes} min
                  </span>
                  <span className="chip !text-[9.5px] hidden sm:inline-flex">
                    {article.length.toLocaleString('fr-FR')} car.
                  </span>
                </>
              )}
            </div>

            <div className="flex items-center gap-1.5">
              <div className="segmented !py-0.5">
                {(Object.keys(THEMES) as BrowserSettings['readerTheme'][]).map((k) => (
                  <button
                    key={k}
                    type="button"
                    data-active={settings.readerTheme === k}
                    onClick={() => onUpdateSettings({ readerTheme: k })}
                  >
                    {THEMES[k].label}
                  </button>
                ))}
              </div>

              <div className="flex items-center gap-1 px-1 rounded-full bg-[var(--fill)] border border-[var(--stroke)]">
                <button
                  type="button"
                  onClick={() =>
                    onUpdateSettings({
                      readerFontSize: Math.max(13, settings.readerFontSize - 1),
                    })
                  }
                  className="icon-btn w-6 h-6"
                  title="Réduire la police"
                >
                  <Minus className="w-3 h-3" />
                </button>
                <span className="text-[10px] font-bold tabular-nums w-6 text-center text-[var(--text-2)]">
                  {settings.readerFontSize}
                </span>
                <button
                  type="button"
                  onClick={() =>
                    onUpdateSettings({
                      readerFontSize: Math.min(28, settings.readerFontSize + 1),
                    })
                  }
                  className="icon-btn w-6 h-6"
                  title="Agrandir la police"
                >
                  <Plus className="w-3 h-3" />
                </button>
              </div>

              <button
                type="button"
                onClick={copyArticle}
                className="icon-btn w-7 h-7"
                title="Copier l'article"
              >
                {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
              </button>
              <button type="button" onClick={onClose} className="icon-btn w-7 h-7" title="Quitter (Échap)">
                <X className="w-4 h-4" />
              </button>
            </div>
          </div>
        </div>

        <div className="max-w-[720px] mx-auto px-6 py-10">
          {loading && (
            <div className="flex flex-col items-center justify-center py-24 gap-3" style={{ color: theme.muted }}>
              <Loader2 className="w-6 h-6 animate-spin text-[var(--accent)]" />
              <p className="text-sm">Extraction de l’article…</p>
            </div>
          )}

          {!loading && !article && (
            <div className="text-center py-24 space-y-3" style={{ color: theme.muted }}>
              <BookOpen className="w-8 h-8 mx-auto opacity-50" />
              <p className="text-sm font-semibold">Impossible d’extraire cet article.</p>
              <p className="text-xs max-w-sm mx-auto">
                Le contenu est peut-être protégé, généré dynamiquement ou l’URL n’est pas une page
                texte.
              </p>
              <button type="button" onClick={onClose} className="btn-glass">
                Revenir à la page
              </button>
            </div>
          )}

          {article && (
            <article className="prose-orakle" style={{ fontSize: `${settings.readerFontSize}px` }}>
              <header className="mb-8 pb-6 border-b" style={{ borderColor: 'rgba(128,128,128,0.22)' }}>
                <div
                  className="text-[10.5px] font-bold uppercase tracking-[0.16em] mb-2.5"
                  style={{ color: theme.muted }}
                >
                  {article.siteName}
                  {article.byline ? ` · ${article.byline}` : ''}
                </div>
                <h1 className="text-[clamp(1.6rem,3.2vw,2.4rem)] font-extrabold leading-[1.15] tracking-[-0.03em] !mt-0">
                  {article.title}
                </h1>
                {article.excerpt && (
                  <p className="mt-3 text-[0.95em] leading-relaxed" style={{ color: theme.muted }}>
                    {article.excerpt}
                  </p>
                )}
                <div className="mt-4 flex items-center gap-2 flex-wrap">
                  <button type="button" onClick={runSummary} className="btn-primary !text-[11px]">
                    {summarizing ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                    ) : (
                      <Sparkles className="w-3.5 h-3.5" />
                    )}
                    Résumer par Orakle
                  </button>
                  <button
                    type="button"
                    onClick={() => onNavigate(article.url)}
                    className="btn-glass !text-[11px]"
                  >
                    <ExternalLink className="w-3 h-3" />
                    Voir la page d’origine
                  </button>
                </div>
              </header>

              {(summary || summarizing) && (
                <div
                  className="mb-8 rounded-2xl p-4 border"
                  style={{
                    background: 'color-mix(in srgb, var(--accent) 9%, transparent)',
                    borderColor: 'color-mix(in srgb, var(--accent) 28%, transparent)',
                  }}
                >
                  <div className="flex items-center gap-2 mb-2">
                    <Wand2 className="w-3.5 h-3.5 text-[var(--accent)]" />
                    <span className="text-[10px] font-bold uppercase tracking-[0.14em] text-[var(--accent)]">
                      Résumé Orakle
                    </span>
                    {summary && (
                      <span className="chip !text-[9px] ml-auto" style={{ color: theme.muted }}>
                        {summary.engine}
                      </span>
                    )}
                  </div>
                  {summarizing ? (
                    <div className="flex items-center gap-2 text-xs" style={{ color: theme.muted }}>
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                      Analyse du texte en cours…
                    </div>
                  ) : (
                    <div className="text-[0.92em] leading-relaxed whitespace-pre-wrap">
                      {summary?.text}
                    </div>
                  )}
                </div>
              )}

              <div dangerouslySetInnerHTML={{ __html: article.html }} />
            </article>
          )}
        </div>
      </div>
    </div>
  );
};
