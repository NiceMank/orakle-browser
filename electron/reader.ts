/**
 * Orakle Reader — extraction d'article lisible (mode Lecture)
 * ------------------------------------------------------------
 * Injection d'un script auto-suffisant dans le <webview> de l'onglet actif.
 * L'algorithme est une variante simplifiée de Readability / Mozilla :
 *  - scoring des conteneurs par densité de texte et ponctuation
 *  - conservation des balises sémantiques utiles
 *  - estimation du temps de lecture
 * Aucun accès réseau, aucune dépendance externe.
 */

export interface ReaderArticle {
  url: string;
  title: string;
  byline: string;
  siteName: string;
  excerpt: string;
  html: string;
  textContent: string;
  length: number;
  readingMinutes: number;
}

/** Script sérialisé, exécuté via webContents.executeJavaScript */
export const READER_EXTRACTOR = `(() => {
  const BLACKLIST = new Set(['SCRIPT','STYLE','NOSCRIPT','IFRAME','SVG','CANVAS','FORM','NAV','FOOTER','ASIDE','HEADER','BUTTON','INPUT','SELECT','TEXTAREA','VIDEO','AUDIO','FIGURE']);
  const KEEP = new Set(['P','H1','H2','H3','H4','H5','H6','UL','OL','LI','BLOCKQUOTE','PRE','CODE','A','IMG','STRONG','EM','B','I','BR','DIV','SPAN','TABLE','THEAD','TBODY','TR','TD','TH','HR','SUP','SUB','FIGCAPTION']);

  const textLen = (el) => (el.innerText || el.textContent || '').trim().length;

  // 1. Sélection du meilleur candidat racine
  let best = null;
  let bestScore = 0;
  const candidates = document.querySelectorAll('article, main, [role=main], div, section');
  candidates.forEach((el) => {
    const len = textLen(el);
    if (len < 280) return;
    let paragraphs = 0;
    let linkLen = 0;
    el.querySelectorAll('p').forEach((p) => {
      const l = textLen(p);
      if (l > 40) paragraphs++;
    });
    el.querySelectorAll('a').forEach((a) => (linkLen += textLen(a)));
    const linkDensity = len > 0 ? linkLen / len : 1;
    if (linkDensity > 0.55) return;
    const score = paragraphs * 24 + Math.min(len, 8000) * 0.35 - linkDensity * 400;
    if (score > bestScore) { bestScore = score; best = el; }
  });

  const root = best || document.body;
  if (!root) return null;

  const clean = document.createElement('div');
  const walk = (node, parent) => {
    node.childNodes.forEach((child) => {
      if (child.nodeType === 3) {
        const t = child.textContent;
        if (t && t.trim()) parent.appendChild(document.createTextNode(t));
        return;
      }
      if (child.nodeType !== 1) return;
      const tag = child.tagName.toUpperCase();
      if (BLACKLIST.has(tag)) return;
      if (child.getAttribute && child.getAttribute('aria-hidden') === 'true') return;

      // Suppression des blocs de navigation / partage
      const cls = (child.className && typeof child.className === 'string' ? child.className : '') + ' ' + (child.id || '');
      if (/share|social|comment|sidebar|nav|menu|promo|newsletter|subscribe|cookie|banner|advert/i.test(cls) && textLen(child) < 400) return;

      if (!KEEP.has(tag)) { walk(child, parent); return; }

      let el;
      if (tag === 'A') {
        el = document.createElement('a');
        el.setAttribute('href', child.getAttribute('href') || '#');
        el.setAttribute('target', '_blank');
        el.setAttribute('rel', 'noreferrer noopener');
      } else if (tag === 'IMG') {
        const src = child.currentSrc || child.src;
        if (!src || /1x1|pixel|spacer|blank/i.test(src)) return;
        el = document.createElement('img');
        el.setAttribute('src', src);
        el.setAttribute('loading', 'lazy');
        const alt = child.getAttribute('alt');
        if (alt) el.setAttribute('alt', alt);
        parent.appendChild(el);
        return;
      } else {
        el = document.createElement(tag.toLowerCase());
      }
      if (tag === 'PRE' || tag === 'CODE') el.textContent = child.textContent;
      else walk(child, el);
      if (el.childNodes.length > 0) parent.appendChild(el);
    });
  };
  walk(root, clean);

  const textContent = (clean.innerText || clean.textContent || '').replace(/\\n{3,}/g, '\\n\\n').trim();
  const meta = (name) => {
    const el = document.querySelector('meta[name="' + name + '"], meta[property="' + name + '"]');
    return el ? el.getAttribute('content') || '' : '';
  };

  const h1 = document.querySelector('h1');
  const words = textContent.split(/\\s+/).filter(Boolean).length;

  return {
    url: location.href,
    title: (h1 && h1.innerText.trim()) || meta('og:title') || document.title || '',
    byline: meta('author') || meta('article:author') || '',
    siteName: meta('og:site_name') || location.hostname,
    excerpt: meta('description') || meta('og:description') || textContent.slice(0, 220),
    html: clean.innerHTML,
    textContent: textContent.slice(0, 60000),
    length: textContent.length,
    readingMinutes: Math.max(1, Math.round(words / 220)),
  };
})()`;

/** Résumé de secours 100 % local (aucun réseau, aucun modèle) */
export function offlineSummarize(text: string, maxSentences = 5): string {
  const clean = text.replace(/\s+/g, ' ').trim();
  if (!clean) return '';
  const sentences = clean
    .split(/(?<=[.!?])\s+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 45 && s.length < 400);
  if (sentences.length === 0) return clean.slice(0, 600);

  const stop = new Set(
    ('le la les de des du un une et en à au aux ce cet cette ces que qui quoi dont ou où il elle ils elles nous vous ils sont est a ont ' +
      'the of and to in is are was were for with that this it on as at by from be or an')
      .split(' ')
  );
  const freq: Record<string, number> = {};
  clean
    .toLowerCase()
    .replace(/[^\p{L}\s]/gu, ' ')
    .split(/\s+/)
    .filter((w) => w.length > 3 && !stop.has(w))
    .forEach((w) => (freq[w] = (freq[w] || 0) + 1));

  const scored = sentences.map((s, i) => {
    const words = s.toLowerCase().replace(/[^\p{L}\s]/gu, ' ').split(/\s+/);
    const score =
      words.reduce((acc, w) => acc + (freq[w] || 0), 0) / Math.sqrt(words.length || 1) +
      (i === 0 ? 3 : 0);
    return { s, score, i };
  });

  return scored
    .sort((a, b) => b.score - a.score)
    .slice(0, maxSentences)
    .sort((a, b) => a.i - b.i)
    .map((x) => x.s)
    .join(' ');
}
