import { SearchEngineId } from '../types/browser';

export const SEARCH_ENGINES: Record<
  SearchEngineId,
  { name: string; badge: string; color: string; searchUrl: string; homeUrl: string }
> = {
  duckduckgo: {
    name: 'DuckDuckGo',
    badge: 'DDG',
    color: '#de5833',
    searchUrl: 'https://duckduckgo.com/?q=',
    homeUrl: 'https://duckduckgo.com',
  },
  brave: {
    name: 'Brave Search',
    badge: 'BRV',
    color: '#fb542b',
    searchUrl: 'https://search.brave.com/search?q=',
    homeUrl: 'https://search.brave.com',
  },
  google: {
    name: 'Google',
    badge: 'G',
    color: '#4285f4',
    searchUrl: 'https://www.google.com/search?q=',
    homeUrl: 'https://www.google.com',
  },
  qwant: {
    name: 'Qwant',
    badge: 'QW',
    color: '#5c4b99',
    searchUrl: 'https://www.qwant.com/?q=',
    homeUrl: 'https://www.qwant.com',
  },
  startpage: {
    name: 'Startpage',
    badge: 'SP',
    color: '#6573ff',
    searchUrl: 'https://www.startpage.com/do/dsearch?query=',
    homeUrl: 'https://www.startpage.com',
  },
};

const BANG_SHORTCUTS: Record<string, string> = {
  '!g': 'https://www.google.com/search?q=',
  '!ddg': 'https://duckduckgo.com/?q=',
  '!br': 'https://search.brave.com/search?q=',
  '!gh': 'https://github.com/search?q=',
  '!yt': 'https://www.youtube.com/results?search_query=',
  '!w': 'https://fr.wikipedia.org/w/index.php?search=',
  '!mdn': 'https://developer.mozilla.org/fr/search?q=',
  '!npm': 'https://www.npmjs.com/search?q=',
};

export function resolveOmniboxInput(rawInput: string, engine: SearchEngineId = 'duckduckgo'): string {
  const input = rawInput.trim();
  if (!input) return 'orakle://newtab';

  // Internal Orakle URLs
  if (input.startsWith('orakle://')) {
    return input.toLowerCase();
  }

  // Bang shortcuts (!yt, !gh, !w, !g, etc.)
  const firstSpace = input.indexOf(' ');
  if (firstSpace > 1) {
    const prefix = input.slice(0, firstSpace).toLowerCase();
    const rest = input.slice(firstSpace + 1).trim();
    if (BANG_SHORTCUTS[prefix] && rest) {
      return `${BANG_SHORTCUTS[prefix]}${encodeURIComponent(rest)}`;
    }
  }

  // Explicit HTTP/HTTPS protocol
  if (/^https?:\/\//i.test(input)) {
    return input;
  }

  // Localhost or IPv4 address
  if (/^(localhost|127\.\d+\.\d+\.\d+|192\.168\.\d+\.\d+|10\.\d+\.\d+\.\d+)(:\d+)?(\/.*)?$/i.test(input)) {
    return `http://${input}`;
  }

  // Domain-like pattern (no spaces, contains at least one dot and valid TLD)
  const isDomainLike =
    !input.includes(' ') &&
    /^[a-zA-Z0-9-]+(\.[a-zA-Z0-9-]+)+([/?#].*)?$/.test(input);

  if (isDomainLike) {
    return `https://${input}`;
  }

  // Fallback to selected search engine
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
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'Ko', 'Mo', 'Go'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}
