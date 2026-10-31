import { SearchEngineId } from '../types/browser';

export const SEARCH_ENGINES: Record<
  SearchEngineId,
  { name: string; badge: string; color: string; searchUrl: string; homeUrl: string; hint?: string }
> = {
  duckduckgo: {
    name: 'DuckDuckGo',
    badge: 'DDG',
    color: '#de5833',
    searchUrl: 'https://duckduckgo.com/?q=',
    homeUrl: 'https://duckduckgo.com',
    hint: 'Sans pistage',
  },
  brave: {
    name: 'Brave Search',
    badge: 'BRV',
    color: '#fb542b',
    searchUrl: 'https://search.brave.com/search?q=',
    homeUrl: 'https://search.brave.com',
    hint: 'Index indépendant',
  },
  google: {
    name: 'Google',
    badge: 'G',
    color: '#4285f4',
    searchUrl: 'https://www.google.com/search?q=',
    homeUrl: 'https://www.google.com',
    hint: 'Le plus large',
  },
  qwant: {
    name: 'Qwant',
    badge: 'QW',
    color: '#5c4b99',
    searchUrl: 'https://www.qwant.com/?q=',
    homeUrl: 'https://www.qwant.com',
    hint: 'Européen',
  },
  startpage: {
    name: 'Startpage',
    badge: 'SP',
    color: '#6573ff',
    searchUrl: 'https://www.startpage.com/do/dsearch?query=',
    homeUrl: 'https://www.startpage.com',
    hint: 'Résultats Google anonymes',
  },
  perplexity: {
    name: 'Perplexity',
    badge: 'PPX',
    color: '#20b8cd',
    searchUrl: 'https://www.perplexity.ai/search?q=',
    homeUrl: 'https://www.perplexity.ai',
    hint: 'Réponses IA',
  },
};

/** Bangs rapides universalisés dans l'Omnibox et la palette */
export const BANG_SHORTCUTS: Record<string, { url: string; label: string; emoji: string }> = {
  '!g': { url: 'https://www.google.com/search?q=', label: 'Google', emoji: '🔍' },
  '!ddg': { url: 'https://duckduckgo.com/?q=', label: 'DuckDuckGo', emoji: '🦆' },
  '!br': { url: 'https://search.brave.com/search?q=', label: 'Brave', emoji: '🦁' },
  '!ai': { url: 'https://www.perplexity.ai/search?q=', label: 'Perplexity IA', emoji: '✨' },
  '!gh': { url: 'https://github.com/search?q=', label: 'GitHub', emoji: '🐙' },
  '!yt': { url: 'https://www.youtube.com/results?search_query=', label: 'YouTube', emoji: '▶️' },
  '!w': { url: 'https://fr.wikipedia.org/w/index.php?search=', label: 'Wikipédia FR', emoji: '📚' },
  '!mdn': { url: 'https://developer.mozilla.org/fr/search?q=', label: 'MDN', emoji: '📘' },
  '!npm': { url: 'https://www.npmjs.com/search?q=', label: 'npm', emoji: '📦' },
  '!so': { url: 'https://stackoverflow.com/search?q=', label: 'Stack Overflow', emoji: '🧠' },
  '!cve': { url: 'https://www.cve.org/CVERecord/SearchResults?query=', label: 'CVE', emoji: '🛡️' },
  '!wiki': { url: 'https://en.wikipedia.org/w/index.php?search=', label: 'Wikipedia EN', emoji: '🌍' },
  '!x': { url: 'https://x.com/search?q=', label: 'X / Twitter', emoji: '🐦' },
  '!li': { url: 'https://www.linkedin.com/search/results/all/?keywords=', label: 'LinkedIn', emoji: '💼' },
  '!rd': { url: 'https://www.reddit.com/search/?q=', label: 'Reddit', emoji: '👽' },
  '!am': { url: 'https://www.amazon.fr/s?k=', label: 'Amazon', emoji: '🛒' },
};

export interface ParsedInput {
  kind: 'url' | 'search' | 'bang' | 'internal';
  target: string;
  label: string;
  emoji: string;
}

/** Analyse l'entrée Omnibox et renvoie un descriptif exploitable par l'UI */
export function parseOmniboxInput(
  rawInput: string,
  engine: SearchEngineId = 'duckduckgo'
): ParsedInput {
  const input = rawInput.trim();
  if (!input) {
    return { kind: 'internal', target: 'orakle://newtab', label: 'Nouvel onglet', emoji: '✨' };
  }
  if (input.startsWith('orakle://')) {
    return { kind: 'internal', target: input.toLowerCase(), label: 'Page interne Orakle', emoji: '✨' };
  }
  const firstSpace = input.indexOf(' ');
  if (firstSpace > 1) {
    const prefix = input.slice(0, firstSpace).toLowerCase();
    const rest = input.slice(firstSpace + 1).trim();
    const bang = BANG_SHORTCUTS[prefix];
    if (bang && rest) {
      return {
        kind: 'bang',
        target: `${bang.url}${encodeURIComponent(rest)}`,
        label: `${bang.label} — « ${rest} »`,
        emoji: bang.emoji,
      };
    }
  }
  const target = resolveOmniboxInput(input, engine);
  if (/^https?:\/\//i.test(target)) {
    return { kind: 'url', target, label: target.replace(/^https?:\/\//, ''), emoji: '🌐' };
  }
  return {
    kind: 'search',
    target,
    label: `${SEARCH_ENGINES[engine].name} — « ${input} »`,
    emoji: '🔍',
  };
}

export function resolveOmniboxInput(rawInput: string, engine: SearchEngineId = 'duckduckgo'): string {
  const input = rawInput.trim();
  if (!input) return 'orakle://newtab';

  if (input.startsWith('orakle://')) {
    return input.toLowerCase();
  }

  // Bang shortcuts (!yt, !gh, !w, !g, etc.)
  const firstSpace = input.indexOf(' ');
  if (firstSpace > 1) {
    const prefix = input.slice(0, firstSpace).toLowerCase();
    const rest = input.slice(firstSpace + 1).trim();
    if (BANG_SHORTCUTS[prefix] && rest) {
      return `${BANG_SHORTCUTS[prefix].url}${encodeURIComponent(rest)}`;
    }
  }

  if (/^https?:\/\//i.test(input)) {
    return input;
  }

  if (/^(localhost|127\.\d+\.\d+\.\d+|192\.168\.\d+\.\d+|10\.\d+\.\d+\.\d+)(:\d+)?(\/.*)?$/i.test(input)) {
    return `http://${input}`;
  }

  const isDomainLike =
    !input.includes(' ') && /^[a-zA-Z0-9-]+(\.[a-zA-Z0-9-]+)+([/?#].*)?$/.test(input);

  if (isDomainLike) {
    return `https://${input}`;
  }

  const searchBase = SEARCH_ENGINES[engine]?.searchUrl || SEARCH_ENGINES.duckduckgo.searchUrl;
  return `${searchBase}${encodeURIComponent(input)}`;
}

export function getSecurityState(url: string): 'secure' | 'insecure' | 'internal' {
  if (!url || url.startsWith('orakle://') || url === 'about:blank') {
    return 'internal';
  }
  if (url.startsWith('https://')) {
    return 'secure';
  }
  return 'insecure';
}

export function extractHostname(url: string): string {
  if (!url || url.startsWith('orakle://')) return 'orakle';
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}

/** URL compacte pour l'affichage (domaine + chemin court) */
export function prettyUrl(url: string, max = 64): string {
  if (!url || url.startsWith('orakle://')) return 'orakle://newtab';
  try {
    const u = new URL(url);
    const path = (u.pathname === '/' ? '' : u.pathname) + (u.search ? u.search.slice(0, 24) : '');
    const full = `${u.hostname.replace(/^www\./, '')}${path}`;
    return full.length > max ? `${full.slice(0, max)}…` : full;
  } catch {
    return url;
  }
}

export function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'Ko', 'Mo', 'Go'];
  const i = Math.min(sizes.length - 1, Math.floor(Math.log(bytes) / Math.log(k)));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

export function formatLatency(ms: number): string {
  if (ms <= 0) return '—';
  if (ms < 1000) return `${Math.round(ms)} ms`;
  return `${(ms / 1000).toFixed(2)} s`;
}

export function relativeTime(timestamp: number): string {
  const diff = Date.now() - timestamp;
  const mins = Math.round(diff / 60000);
  if (mins < 1) return "à l'instant";
  if (mins < 60) return `il y a ${mins} min`;
  const hours = Math.round(mins / 60);
  if (hours < 24) return `il y a ${hours} h`;
  const days = Math.round(hours / 24);
  if (days < 7) return `il y a ${days} j`;
  return new Date(timestamp).toLocaleDateString('fr-FR', { day: 'numeric', month: 'short' });
}

/** Groupement d'historique par jour */
export function groupByDay<T extends { visitedAt: number }>(items: T[]): [string, T[]][] {
  const buckets = new Map<string, T[]>();
  const today = new Date();
  const keyOf = (ts: number) => new Date(ts).toDateString();
  const todayKey = today.toDateString();
  const yesterdayKey = new Date(today.getTime() - 86400000).toDateString();

  items.forEach((item) => {
    let label: string;
    const k = keyOf(item.visitedAt);
    if (k === todayKey) label = "Aujourd'hui";
    else if (k === yesterdayKey) label = 'Hier';
    else
      label = new Date(item.visitedAt).toLocaleDateString('fr-FR', {
        weekday: 'long',
        day: 'numeric',
        month: 'long',
      });
    if (!buckets.has(label)) buckets.set(label, []);
    buckets.get(label)!.push(item);
  });

  return Array.from(buckets.entries());
}

/** Distance de correspondance floue simple pour la palette de commandes */
export function fuzzyScore(query: string, text: string): number {
  const q = query.toLowerCase().trim();
  const t = text.toLowerCase();
  if (!q) return 1;
  if (t.includes(q)) return 100 - t.indexOf(q);
  let score = 0;
  let ti = 0;
  for (const ch of q) {
    const idx = t.indexOf(ch, ti);
    if (idx === -1) return 0;
    score += idx === ti ? 3 : 1;
    ti = idx + 1;
  }
  return score;
}
