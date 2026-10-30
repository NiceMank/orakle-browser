import { spawn } from 'node:child_process';
import http from 'node:http';

const PORT = 5188;
const DEV_URL = `http://localhost:${PORT}`;

function waitForServer(url, timeoutMs = 15000) {
  const start = Date.now();
  return new Promise((resolve, reject) => {
    const check = () => {
      http
        .get(url, (res) => {
          res.resume();
          resolve();
        })
        .on('error', () => {
          if (Date.now() - start > timeoutMs) {
            reject(new Error(`Timeout waiting for ${url}`));
          } else {
            setTimeout(check, 200);
          }
        });
    };
    check();
  });
}

console.log('[Orakle Dev] Démarrage du serveur Vite...');
const viteProc = spawn('npx', ['vite', '--port', String(PORT), '--strictPort'], {
  stdio: 'inherit',
  shell: true,
});

try {
  await waitForServer(DEV_URL);
  console.log('[Orakle Dev] Serveur Vite prêt ! Lancement d’Orakle Browser (Electron)...');

  const electronProc = spawn('npx', ['electron', '.', '--no-sandbox'], {
    stdio: 'inherit',
    shell: true,
    env: {
      ...process.env,
      VITE_DEV_SERVER_URL: DEV_URL,
    },
  });

  electronProc.on('close', (code) => {
    viteProc.kill();
    process.exit(code ?? 0);
  });
} catch (err) {
  console.error('[Orakle Dev] Erreur :', err);
  viteProc.kill();
  process.exit(1);
}
