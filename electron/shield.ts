import { Session } from 'electron';

export interface BlockedEntry {
  id: string;
  domain: string;
  url: string;
  category: 'Publicité' | 'Traqueur' | 'Télémétrie' | 'Paramètre URL';
  timestamp: number;
}

export interface ShieldStats {
  enabled: boolean;
  stripTrackingParams: boolean;
  sendDoNotTrack: boolean;
  userAgentProfile: string;
  totalBlocked: number;
  trackersBlocked: number;
  adsBlocked: number;
  paramsStripped: number;
  estimatedBytesSaved: number;
  whitelistedHosts: string[];
  recentBlocked: BlockedEntry[];
  topBlockedDomains: Record<string, number>;
}

const AD_AND_TRACKER_DOMAINS: Record<string, 'Publicité' | 'Traqueur' | 'Télémétrie'> = {
  'doubleclick.net': 'Publicité',
  'googlesyndication.com': 'Publicité',
  'googleadservices.com': 'Publicité',
  'adservice.google.com': 'Publicité',
  'pagead2.googlesyndication.com': 'Publicité',
  'google-analytics.com': 'Traqueur',
  'googletagmanager.com': 'Traqueur',
  'googletagservices.com': 'Publicité',
  'connect.facebook.net': 'Traqueur',
  'pixel.facebook.com': 'Traqueur',
  'an.facebook.com': 'Publicité',
  'analytics.tiktok.com': 'Traqueur',
  'ads-twitter.com': 'Publicité',
  'static.ads-twitter.com': 'Publicité',
  'analytics.twitter.com': 'Traqueur',
  'bat.bing.com': 'Traqueur',
  'criteo.com': 'Publicité',
  'criteo.net': 'Publicité',
  'taboola.com': 'Publicité',
  'trc.taboola.com': 'Publicité',
  'outbrain.com': 'Publicité',
  'widgets.outbrain.com': 'Publicité',
  'adnxs.com': 'Publicité',
  'ib.adnxs.com': 'Publicité',
  'rubiconproject.com': 'Publicité',
  'pubmatic.com': 'Publicité',
  'openx.net': 'Publicité',
  'smartadserver.com': 'Publicité',
  'amazon-adsystem.com': 'Publicité',
  'aax.amazon-adsystem.com': 'Publicité',
  'moatads.com': 'Traqueur',
  'scorecardresearch.com': 'Traqueur',
  'sb.scorecardresearch.com': 'Traqueur',
  'quantserve.com': 'Traqueur',
  'hotjar.com': 'Télémétrie',
  'static.hotjar.com': 'Télémétrie',
  'mouseflow.com': 'Télémétrie',
  'fullstory.com': 'Télémétrie',
  'clarity.ms': 'Télémétrie',
  'mc.yandex.ru': 'Traqueur',
  'mixpanel.com': 'Télémétrie',
  'api.mixpanel.com': 'Télémétrie',
  'segment.io': 'Télémétrie',
  'api.segment.io': 'Télémétrie',
  'amplitude.com': 'Télémétrie',
  'api.amplitude.com': 'Télémétrie',
  'bugsnag.com': 'Télémétrie',
  'sentry.io': 'Télémétrie',
  'newrelic.com': 'Télémétrie',
  'nr-data.net': 'Télémétrie',
  'chartbeat.com': 'Traqueur',
  'chartbeat.net': 'Traqueur',
  'optimizely.com': 'Traqueur',
  'pardot.com': 'Traqueur',
  'hubspot.com': 'Traqueur',
  'hs-analytics.net': 'Traqueur',
  'hs-scripts.com': 'Traqueur',
  'adroll.com': 'Publicité',
  'teads.tv': 'Publicité',
  'casalemedia.com': 'Publicité',
  'indexww.com': 'Publicité',
  'bidswitch.net': 'Publicité',
  'sharethrough.com': 'Publicité',
  'medianet.com': 'Publicité',
  'popads.net': 'Publicité',
  'propellerads.com': 'Publicité',
  'adsterra.com': 'Publicité',
};

const TRACKING_QUERY_PARAMS = new Set([
  'utm_source',
  'utm_medium',
  'utm_campaign',
  'utm_term',
  'utm_content',
  'utm_id',
  'fbclid',
  'gclid',
  'dclid',
  'msclkid',
  'twclid',
  'yclid',
  'mc_cid',
  'mc_eid',
  '_ga',
  '_gl',
  'igshid',
  'si',
]);

export const USER_AGENT_PROFILES: Record<string, { label: string; ua: string }> = {
  standard: {
    label: 'Orakle Standard (Linux x64)',
    ua: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Orakle/1.0',
  },
  'chrome-win': {
    label: 'Chrome 131 — Windows 11',
    ua: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
  },
  'firefox-linux': {
    label: 'Firefox 133 — Linux x86_64',
    ua: 'Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0',
  },
  'safari-mac': {
    label: 'Safari 18.1 — macOS Sequoia',
    ua: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_7_1) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15',
  },
  'mobile-ios': {
    label: 'iPhone 16 Pro — iOS 18',
    ua: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1',
  },
};

export class OrakleShield {
  private stats: ShieldStats = {
    enabled: true,
    stripTrackingParams: true,
    sendDoNotTrack: true,
    userAgentProfile: 'standard',
    totalBlocked: 0,
    trackersBlocked: 0,
    adsBlocked: 0,
    paramsStripped: 0,
    estimatedBytesSaved: 0,
    whitelistedHosts: [],
    recentBlocked: [],
    topBlockedDomains: {},
  };

  private onUpdateCallback?: (stats: ShieldStats) => void;

  constructor(initialConfig?: Partial<ShieldStats>, onUpdate?: (stats: ShieldStats) => void) {
    if (initialConfig) {
      this.stats = { ...this.stats, ...initialConfig };
    }
    this.onUpdateCallback = onUpdate;
  }

  public getStats(): ShieldStats {
    return { ...this.stats };
  }

  public updateConfig(patch: Partial<Pick<ShieldStats, 'enabled' | 'stripTrackingParams' | 'sendDoNotTrack' | 'userAgentProfile' | 'whitelistedHosts'>>): ShieldStats {
    this.stats = { ...this.stats, ...patch };
    this.notify();
    return this.getStats();
  }

  public toggleWhitelistHost(host: string): ShieldStats {
    const cleanHost = host.trim().toLowerCase();
    if (!cleanHost) return this.getStats();
    const exists = this.stats.whitelistedHosts.includes(cleanHost);
    this.stats.whitelistedHosts = exists
      ? this.stats.whitelistedHosts.filter((h) => h !== cleanHost)
      : [...this.stats.whitelistedHosts, cleanHost];
    this.notify();
    return this.getStats();
  }

  private matchBlockedDomain(hostname: string): { matched: string; category: 'Publicité' | 'Traqueur' | 'Télémétrie' } | null {
    const lower = hostname.toLowerCase();
    for (const [domain, category] of Object.entries(AD_AND_TRACKER_DOMAINS)) {
      if (lower === domain || lower.endsWith('.' + domain)) {
        return { matched: domain, category };
      }
    }
    return null;
  }

  private recordBlocked(domain: string, url: string, category: BlockedEntry['category'], bytesEstimate = 28500) {
    this.stats.totalBlocked += 1;
    if (category === 'Publicité') {
      this.stats.adsBlocked += 1;
    } else if (category === 'Paramètre URL') {
      this.stats.paramsStripped += 1;
    } else {
      this.stats.trackersBlocked += 1;
    }
    this.stats.estimatedBytesSaved += bytesEstimate;
    this.stats.topBlockedDomains[domain] = (this.stats.topBlockedDomains[domain] || 0) + 1;

    const entry: BlockedEntry = {
      id: `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
      domain,
      url: url.slice(0, 180),
      category,
      timestamp: Date.now(),
    };
    this.stats.recentBlocked = [entry, ...this.stats.recentBlocked.slice(0, 59)];
    this.notify();
  }

  private notify() {
    if (this.onUpdateCallback) {
      this.onUpdateCallback(this.getStats());
    }
  }

  public attachToSession(ses: Session) {
    ses.webRequest.onBeforeRequest((details, callback) => {
      if (!this.stats.enabled) {
        callback({ cancel: false });
        return;
      }

      try {
        const parsed = new URL(details.url);
        if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
          callback({ cancel: false });
          return;
        }

        // Check if initiator or target host is whitelisted
        let initiatorHost = '';
        if (details.referrer) {
          try {
            initiatorHost = new URL(details.referrer).hostname.toLowerCase();
          } catch {
            initiatorHost = '';
          }
        }
        const targetHost = parsed.hostname.toLowerCase();
        if (
          this.stats.whitelistedHosts.includes(targetHost) ||
          (initiatorHost && this.stats.whitelistedHosts.includes(initiatorHost))
        ) {
          callback({ cancel: false });
          return;
        }

        // 1. Domain-level Ad & Tracker Blocking
        const match = this.matchBlockedDomain(targetHost);
        if (match) {
          this.recordBlocked(match.matched, details.url, match.category);
          callback({ cancel: true });
          return;
        }

        // 2. URL Path heuristic for common ad/tracker scripts
        const pathLower = parsed.pathname.toLowerCase();
        if (
          details.resourceType === 'script' &&
          (pathLower.includes('/pagead/') ||
            pathLower.includes('/fbevents.js') ||
            pathLower.includes('/ads/ads.js') ||
            pathLower.endsWith('/analytics.js'))
        ) {
          this.recordBlocked(targetHost, details.url, 'Traqueur');
          callback({ cancel: true });
          return;
        }

        // 3. Strip tracking query parameters on mainFrame / subFrame navigation
        if (
          this.stats.stripTrackingParams &&
          (details.resourceType === 'mainFrame' || details.resourceType === 'subFrame') &&
          parsed.search.length > 1
        ) {
          let strippedCount = 0;
          for (const key of Array.from(parsed.searchParams.keys())) {
            if (TRACKING_QUERY_PARAMS.has(key.toLowerCase())) {
              parsed.searchParams.delete(key);
              strippedCount++;
            }
          }
          if (strippedCount > 0) {
            this.recordBlocked(targetHost, details.url, 'Paramètre URL', 1200);
            callback({ redirectURL: parsed.toString() });
            return;
          }
        }
      } catch {
        // Ignore malformed URLs
      }

      callback({ cancel: false });
    });

    ses.webRequest.onBeforeSendHeaders((details, callback) => {
      const requestHeaders = { ...details.requestHeaders };

      if (this.stats.sendDoNotTrack) {
        requestHeaders['DNT'] = '1';
        requestHeaders['Sec-GPC'] = '1';
      }

      const profile = USER_AGENT_PROFILES[this.stats.userAgentProfile];
      if (profile && this.stats.userAgentProfile !== 'standard') {
        requestHeaders['User-Agent'] = profile.ua;
      }

      callback({ requestHeaders });
    });
  }
}
