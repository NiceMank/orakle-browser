/**
 * Signes vitaux du navigateur (RAM / CPU) — exploités par la barre d'état Aurora.
 * Utilise l'API Performance (memory) quand elle est disponible et un lissage
 * exponentiel pour éviter les valeurs erratiques.
 */
export interface BrowserVitals {
  memoryMb: number;
  memoryLimitMb: number;
  cpuPercent: number;
  cores: number;
  updatedAt: number;
}

export const initialVitals: BrowserVitals = {
  memoryMb: 0,
  memoryLimitMb: 0,
  cpuPercent: 0,
  cores: typeof navigator !== 'undefined' ? navigator.hardwareConcurrency || 0 : 0,
  updatedAt: 0,
};

type PerfMemory = { usedJSHeapSize: number; jsHeapSizeLimit: number };

export function watchBrowserVitals(onUpdate: (v: BrowserVitals) => void, intervalMs = 3000) {
  let smoothedCpu = 0;
  let lastTick = performance.now();
  let lastIdle = 0;

  const measure = () => {
    const now = performance.now();
    const elapsed = now - lastTick;
    lastTick = now;

    const perf = performance as Performance & { memory?: PerfMemory };
    const memoryMb = perf.memory ? Math.round(perf.memory.usedJSHeapSize / 1048576) : 0;
    const memoryLimitMb = perf.memory ? Math.round(perf.memory.jsHeapSizeLimit / 1048576) : 0;

    // Approximation du temps de traitement du thread principal
    const drift = Math.max(0, elapsed - intervalMs);
    const instant = Math.min(100, Math.round((drift / Math.max(1, elapsed)) * 100));
    smoothedCpu = smoothedCpu === 0 ? instant : Math.round(smoothedCpu * 0.65 + instant * 0.35);
    void lastIdle;

    onUpdate({
      memoryMb,
      memoryLimitMb,
      cpuPercent: smoothedCpu,
      cores: navigator.hardwareConcurrency || 0,
      updatedAt: Date.now(),
    });
  };

  measure();
  const id = setInterval(measure, intervalMs);
  return () => clearInterval(id);
}
