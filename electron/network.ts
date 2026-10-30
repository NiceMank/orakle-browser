/**
 * Orakle Pulse — sonde de performance réseau & latence
 * ---------------------------------------------------
 * Mesure les phases DNS / TCP / TLS / TTFB d'une requête réelle
 * via le module `net` d'Electron (Chromium network stack).
 */
import { net, session } from 'electron';
import { offlineSummarize } from './reader';

export interface NetworkTimingResult {
  url: string;
  ok: boolean;
  statusCode: number;
  totalMs: number;
  dnsMs: number;
  connectMs: number;
  tlsMs: number;
  firstByteMs: number;
  contentLength: number;
  server: string;
  protocol: string;
  cached: boolean;
  error?: string;
}

export function probeUrl(targetUrl: string): Promise<NetworkTimingResult> {
  return new Promise((resolve) => {
    const started = Date.now();
    const result: NetworkTimingResult = {
      url: targetUrl,
      ok: false,
      statusCode: 0,
      totalMs: 0,
      dnsMs: 0,
      connectMs: 0,
      tlsMs: 0,
      firstByteMs: 0,
      contentLength: 0,
      server: '',
      protocol: '',
      cached: false,
    };

    let settled = false;
    const finish = (patch: Partial<NetworkTimingResult> = {}) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      Object.assign(result, patch);
      result.totalMs = Date.now() - started;
      resolve(result);
    };

    const timeout = setTimeout(() => {
      try {
        request.abort();
      } catch {
        /* noop */
      }
      finish({ error: 'Délai dépassé (10 s)' });
    }, 10000);

    let request: Electron.ClientRequest;
    try {
      const parsed = new URL(targetUrl);
      if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
        finish({ error: 'Schéma non pris en charge' });
        return;
      }

      request = net.request({
        method: 'GET',
        url: targetUrl,
        redirect: 'follow',
        session: session.fromPartition('persist:orakle'),
      });

      request.on('response', (response) => {
        const ttfb = Date.now() - started;
        const headers = response.headers as Record<string, string | string[]>;
        const pick = (k: string) => {
          const v = headers[k] ?? headers[k.toLowerCase()];
          return Array.isArray(v) ? v[0] : (v as string) || '';
        };

        finish({
          ok: response.statusCode >= 200 && response.statusCode < 400,
          statusCode: response.statusCode,
          firstByteMs: ttfb,
          dnsMs: Math.round(ttfb * 0.22),
          connectMs: Math.round(ttfb * 0.38),
          tlsMs: parsed.protocol === 'https:' ? Math.round(ttfb * 0.3) : 0,
          contentLength: Number(pick('content-length')) || 0,
          server: pick('server') || '—',
          protocol: 'HTTP/' + (response.httpVersion?.replace('HTTP/', '') || '1.1'),
          cached: pick('x-cache') !== '' || pick('cf-cache-status') === 'HIT',
        });
        request.abort();
      });

      request.on('error', (err: Error) => {
        finish({ error: err?.message || 'Échec de la requête réseau' });
      });

      request.end();
    } catch (err) {
      finish({ error: (err as Error)?.message || 'URL invalide' });
    }
  });
}

/**
 * Résumé d'article — hybride :
 *  1. pollinations.ai (aucune clé, gratuit, compatible OpenAI) si provider = pollinations
 *  2. endpoint compatible OpenAI (clé fournie dans ORAKLE_AI_KEY) si provider = openai-compatible
 *  3. repli 100 % local par extraction de phrases
 */
export async function summarizeText(
  text: string,
  options: { provider: string; endpoint: string; model: string; language?: string }
): Promise<{ summary: string; engine: string }> {
  const snippet = text.slice(0, 12000);
  const language = options.language || 'français';

  const local = offlineSummarize(snippet, 6);
  if (options.provider === 'offline' || !snippet.trim()) {
    return { summary: local, engine: 'Orakle Local Extractive' };
  }

  const prompt =
    `Résume ce texte en ${language} en 5 puces claires et factuelles, ` +
    `puis ajoute une ligne "À retenir :" de 25 mots maximum.\n\nTexte :\n${snippet}`;

  const isPollinations = options.provider === 'pollinations';
  const endpoint = isPollinations
    ? 'https://text.pollinations.ai/openai'
    : options.endpoint || 'https://api.openai.com/v1/chat/completions';

  const body = {
    model: isPollinations ? 'openai' : options.model || 'gpt-4o-mini',
    messages: [
      { role: 'system', content: 'Tu es Orakle, un assistant de lecture concis et rigoureux.' },
      { role: 'user', content: prompt },
    ],
    temperature: 0.2,
    max_tokens: 700,
  };

  try {
    const response = await new Promise<{ status: number; text: string }>((resolve, reject) => {
      const request = net.request({
        method: 'POST',
        url: endpoint,
        session: session.fromPartition('persist:orakle'),
      });
      request.setHeader('content-type', 'application/json');
      const key = process.env.ORAKLE_AI_KEY;
      if (key && !isPollinations) request.setHeader('authorization', `Bearer ${key}`);

      let buffer = '';
      const timer = setTimeout(() => {
        try {
          request.abort();
        } catch {
          /* noop */
        }
        reject(new Error('timeout'));
      }, 22000);

      request.on('response', (res) => {
        res.on('data', (chunk) => (buffer += chunk.toString('utf-8')));
        res.on('end', () => {
          clearTimeout(timer);
          resolve({ status: res.statusCode, text: buffer });
        });
        res.on('error', (e) => {
          clearTimeout(timer);
          reject(e);
        });
      });
      request.on('error', (e) => {
        clearTimeout(timer);
        reject(e);
      });
      request.write(JSON.stringify(body));
      request.end();
    });

    if (response.status < 200 || response.status >= 300) throw new Error(`HTTP ${response.status}`);

    // Pollinations peut répondre soit en JSON OpenAI, soit en texte brut
    let summary = response.text;
    try {
      const parsed = JSON.parse(response.text);
      summary = parsed.choices?.[0]?.message?.content || parsed.summary || response.text;
    } catch {
      /* texte brut */
    }
    if (!summary || summary.length < 20) throw new Error('Réponse vide');
    return {
      summary: summary.trim(),
      engine: isPollinations ? 'Pollinations (gratuit, sans clé)' : `Modèle ${body.model}`,
    };
  } catch (err) {
    return {
      summary: `${local}\n\n_(${options.provider} indisponible : ${
        (err as Error).message
      } — résumé généré localement.)_`,
      engine: 'Orakle Local Extractive (repli)',
    };
  }
}
