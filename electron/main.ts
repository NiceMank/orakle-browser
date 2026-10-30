import {
  app,
  BrowserWindow,
  ipcMain,
  session,
  Menu,
  shell,
  clipboard,
  net,
} from 'electron';
import path from 'node:path';
import { OrakleShield } from './shield';
import { StorageManager, BrowserSettings } from './storage';

// Handle Linux sandbox environments safely
app.commandLine.appendSwitch('enable-features', 'OverlayScrollbar,SmoothScrolling');

let mainWindow: BrowserWindow | null = null;
let storage: StorageManager;
let shield: OrakleShield;

export interface DownloadItemInfo {
  id: string;
  filename: string;
  url: string;
  savePath: string;
  receivedBytes: number;
  totalBytes: number;
  state: 'progressing' | 'completed' | 'cancelled' | 'interrupted';
  startedAt: number;
}

const downloadsList: DownloadItemInfo[] = [];

export interface SecurityAuditResult {
  url: string;
  statusCode: number;
  isHttps: boolean;
  score: number;
  grade: 'A+' | 'A' | 'B' | 'C' | 'D' | 'F';
  headers: Record<string, string>;
  checks: {
    name: string;
    header: string;
    present: boolean;
    value: string;
    description: string;
  }[];
}

function setupDownloadListener(ses: Electron.Session) {
  ses.on('will-download', (_event, item) => {
    const id = `dl-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    const filename = item.getFilename();
    const downloadInfo: DownloadItemInfo = {
      id,
      filename,
      url: item.getURL(),
      savePath: item.getSavePath() || path.join(app.getPath('downloads'), filename),
      receivedBytes: 0,
      totalBytes: item.getTotalBytes(),
      state: 'progressing',
      startedAt: Date.now(),
    };

    downloadsList.unshift(downloadInfo);
    broadcastDownloads();

    item.on('updated', (_e, state) => {
      downloadInfo.receivedBytes = item.getReceivedBytes();
      downloadInfo.totalBytes = item.getTotalBytes();
      downloadInfo.savePath = item.getSavePath();
      downloadInfo.state = state === 'interrupted' ? 'interrupted' : 'progressing';
      broadcastDownloads();
    });

    item.once('done', (_e, state) => {
      downloadInfo.receivedBytes = item.getReceivedBytes();
      downloadInfo.savePath = item.getSavePath();
      downloadInfo.state = state;
      broadcastDownloads();
    });
  });
}

function broadcastDownloads() {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.webContents.send('downloads:updated', downloadsList.slice(0, 40));
  }
}

function configurePermissions(ses: Electron.Session) {
  const allowedPermissions = new Set([
    'fullscreen',
    'clipboard-sanitized-write',
    'mediaKeySystem',
    'notifications',
  ]);

  ses.setPermissionRequestHandler((_webContents, permission, callback) => {
    callback(allowedPermissions.has(permission));
  });
}

function createWindow() {
  const isDev = process.env.VITE_DEV_SERVER_URL !== undefined;

  mainWindow = new BrowserWindow({
    width: 1440,
    height: 920,
    minWidth: 900,
    minHeight: 600,
    frame: false,
    titleBarStyle: 'hidden',
    backgroundColor: '#090a10',
    show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      webviewTag: true,
      sandbox: false,
      spellcheck: true,
    },
  });

  mainWindow.once('ready-to-show', () => {
    mainWindow?.show();
  });

  mainWindow.on('maximize', () => {
    mainWindow?.webContents.send('window:maximized-change', true);
  });

  mainWindow.on('unmaximize', () => {
    mainWindow?.webContents.send('window:maximized-change', false);
  });

  if (isDev && process.env.VITE_DEV_SERVER_URL) {
    mainWindow.loadURL(process.env.VITE_DEV_SERVER_URL);
  } else {
    mainWindow.loadFile(path.join(__dirname, '../dist/index.html'));
  }
}

// Configure every guest <webview> created by the browser
app.on('web-contents-created', (_event, contents) => {
  if (contents.getType() === 'webview') {
    // Route target="_blank" and window.open() into a new Orakle tab
    contents.setWindowOpenHandler(({ url, disposition }) => {
      if (url && (url.startsWith('http://') || url.startsWith('https://'))) {
        mainWindow?.webContents.send('tabs:open-from-main', {
          url,
          background: disposition === 'background-tab',
        });
      }
      return { action: 'deny' };
    });

    // Attach rich native context menu inside webviews
    contents.on('context-menu', (_e, params) => {
      const menuTemplate: Electron.MenuItemConstructorOptions[] = [
        {
          label: 'Précédent',
          enabled: contents.navigationHistory.canGoBack(),
          click: () => contents.navigationHistory.goBack(),
        },
        {
          label: 'Suivant',
          enabled: contents.navigationHistory.canGoForward(),
          click: () => contents.navigationHistory.goForward(),
        },
        {
          label: 'Actualiser la page',
          click: () => contents.reload(),
        },
        { type: 'separator' },
      ];

      if (params.linkURL) {
        menuTemplate.push(
          {
            label: 'Ouvrir le lien dans un nouvel onglet',
            click: () => {
              mainWindow?.webContents.send('tabs:open-from-main', {
                url: params.linkURL,
                background: false,
              });
            },
          },
          {
            label: 'Ouvrir le lien en onglet Fantôme (Privé)',
            click: () => {
              mainWindow?.webContents.send('tabs:open-from-main', {
                url: params.linkURL,
                isPrivate: true,
              });
            },
          },
          {
            label: "Copier l'adresse du lien",
            click: () => clipboard.writeText(params.linkURL),
          },
          { type: 'separator' }
        );
      }

      if (params.selectionText) {
        menuTemplate.push(
          { role: 'copy', label: 'Copier la sélection' },
          {
            label: `Rechercher "${params.selectionText.slice(0, 28)}" dans un nouvel onglet`,
            click: () => {
              const query = encodeURIComponent(params.selectionText);
              mainWindow?.webContents.send('tabs:open-from-main', {
                url: `https://duckduckgo.com/?q=${query}`,
                background: false,
              });
            },
          },
          { type: 'separator' }
        );
      }

      if (params.isEditable) {
        menuTemplate.push(
          { role: 'undo', label: 'Annuler' },
          { role: 'redo', label: 'Rétablir' },
          { type: 'separator' },
          { role: 'cut', label: 'Couper' },
          { role: 'copy', label: 'Copier' },
          { role: 'paste', label: 'Coller' },
          { role: 'selectAll', label: 'Tout sélectionner' },
          { type: 'separator' }
        );
      }

      menuTemplate.push(
        {
          label: "Copier l'URL de la page",
          click: () => clipboard.writeText(contents.getURL()),
        },
        {
          label: "Inspecter l'élément (DevTools)",
          click: () => contents.inspectElement(params.x, params.y),
        }
      );

      const menu = Menu.buildFromTemplate(menuTemplate);
      menu.popup();
    });
  }
});

async function auditSecurityHeaders(targetUrl: string): Promise<SecurityAuditResult> {
  return new Promise((resolve) => {
    try {
      const parsed = new URL(targetUrl);
      const isHttps = parsed.protocol === 'https:';
      if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
        resolve({
          url: targetUrl,
          statusCode: 200,
          isHttps: true,
          score: 100,
          grade: 'A+',
          headers: {},
          checks: [],
        });
        return;
      }

      const request = net.request({
        method: 'GET',
        url: targetUrl,
        redirect: 'follow',
      });

      const timeout = setTimeout(() => {
        request.abort();
      }, 6000);

      request.on('response', (response) => {
        clearTimeout(timeout);
        const rawHeaders: Record<string, string> = {};
        for (const [k, v] of Object.entries(response.headers)) {
          rawHeaders[k.toLowerCase()] = Array.isArray(v) ? v.join(', ') : String(v);
        }
        // Abort body reading immediately since we only need headers
        request.abort();

        const checks = [
          {
            name: 'Chiffrement HTTPS / TLS',
            header: 'https',
            present: isHttps,
            value: isHttps ? 'Actif (TLS 1.2/1.3)' : 'Absent (HTTP en clair)',
            description: 'Protège les échanges contre toute écoute réseau (MITM).',
          },
          {
            name: 'Content-Security-Policy (CSP)',
            header: 'content-security-policy',
            present: Boolean(rawHeaders['content-security-policy']),
            value: rawHeaders['content-security-policy']?.slice(0, 90) || 'Non défini',
            description: 'Prévient les injections XSS et le chargement de ressources non autorisées.',
          },
          {
            name: 'Strict-Transport-Security (HSTS)',
            header: 'strict-transport-security',
            present: Boolean(rawHeaders['strict-transport-security']),
            value: rawHeaders['strict-transport-security'] || 'Non défini',
            description: 'Force le navigateur à utiliser uniquement des connexions HTTPS.',
          },
          {
            name: 'X-Frame-Options / Frame-Ancestors',
            header: 'x-frame-options',
            present:
              Boolean(rawHeaders['x-frame-options']) ||
              Boolean(rawHeaders['content-security-policy']?.includes('frame-ancestors')),
            value: rawHeaders['x-frame-options'] || (rawHeaders['content-security-policy']?.includes('frame-ancestors') ? 'Via CSP frame-ancestors' : 'Non défini'),
            description: 'Protège contre le détournement de clic (Clickjacking).',
          },
          {
            name: 'X-Content-Type-Options',
            header: 'x-content-type-options',
            present: rawHeaders['x-content-type-options']?.toLowerCase() === 'nosniff',
            value: rawHeaders['x-content-type-options'] || 'Non défini',
            description: 'Empêche le navigateur de deviner le type MIME des fichiers.',
          },
          {
            name: 'Referrer-Policy',
            header: 'referrer-policy',
            present: Boolean(rawHeaders['referrer-policy']),
            value: rawHeaders['referrer-policy'] || 'Non défini',
            description: 'Contrôle la fuite d’URL dans l’en-tête Referer vers les sites tiers.',
          },
          {
            name: 'Permissions-Policy',
            header: 'permissions-policy',
            present: Boolean(rawHeaders['permissions-policy']),
            value: rawHeaders['permissions-policy']?.slice(0, 90) || 'Non défini',
            description: 'Restreint l’accès aux APIs matérielles (caméra, micro, géolocalisation).',
          },
        ];

        let score = isHttps ? 30 : 0;
        if (checks[1].present) score += 20;
        if (checks[2].present) score += 15;
        if (checks[3].present) score += 10;
        if (checks[4].present) score += 10;
        if (checks[5].present) score += 8;
        if (checks[6].present) score += 7;

        let grade: SecurityAuditResult['grade'] = 'F';
        if (score >= 92) grade = 'A+';
        else if (score >= 80) grade = 'A';
        else if (score >= 65) grade = 'B';
        else if (score >= 50) grade = 'C';
        else if (score >= 30) grade = 'D';

        resolve({
          url: targetUrl,
          statusCode: response.statusCode,
          isHttps,
          score,
          grade,
          headers: rawHeaders,
          checks,
        });
      });

      request.on('error', () => {
        clearTimeout(timeout);
        resolve({
          url: targetUrl,
          statusCode: 0,
          isHttps,
          score: isHttps ? 30 : 0,
          grade: isHttps ? 'D' : 'F',
          headers: {},
          checks: [],
        });
      });

      request.end();
    } catch {
      resolve({
        url: targetUrl,
        statusCode: 0,
        isHttps: false,
        score: 0,
        grade: 'F',
        headers: {},
        checks: [],
      });
    }
  });
}

app.whenReady().then(() => {
  storage = new StorageManager();
  const initialData = storage.getAll();

  shield = new OrakleShield(
    {
      enabled: initialData.settings.shieldEnabled,
      stripTrackingParams: initialData.settings.stripTrackingParams,
      sendDoNotTrack: initialData.settings.sendDoNotTrack,
      userAgentProfile: initialData.settings.userAgentProfile,
      whitelistedHosts: initialData.whitelistedHosts,
    },
    (stats) => {
      if (mainWindow && !mainWindow.isDestroyed()) {
        mainWindow.webContents.send('shield:updated', stats);
      }
    }
  );

  // Attach Orakle Shield & Download manager to persistent and incognito partitions
  const mainSession = session.fromPartition('persist:orakle');
  const incognitoSession = session.fromPartition('orakle-incognito');

  shield.attachToSession(mainSession);
  shield.attachToSession(incognitoSession);
  setupDownloadListener(mainSession);
  setupDownloadListener(incognitoSession);
  configurePermissions(mainSession);
  configurePermissions(incognitoSession);

  // IPC Handlers — Window Controls
  ipcMain.handle('window:minimize', () => mainWindow?.minimize());
  ipcMain.handle('window:maximize', () => {
    if (!mainWindow) return false;
    if (mainWindow.isMaximized()) {
      mainWindow.unmaximize();
      return false;
    } else {
      mainWindow.maximize();
      return true;
    }
  });
  ipcMain.handle('window:close', () => mainWindow?.close());
  ipcMain.handle('window:is-maximized', () => mainWindow?.isMaximized() ?? false);

  // IPC Handlers — State & Storage
  ipcMain.handle('storage:get-all', () => storage.getAll());
  ipcMain.handle('storage:update-settings', (_e, patch: Partial<BrowserSettings>) => {
    const updated = storage.updateSettings(patch);
    shield.updateConfig({
      enabled: updated.shieldEnabled,
      stripTrackingParams: updated.stripTrackingParams,
      sendDoNotTrack: updated.sendDoNotTrack,
      userAgentProfile: updated.userAgentProfile,
    });
    return updated;
  });
  ipcMain.handle('storage:add-bookmark', (_e, bm) => storage.addBookmark(bm));
  ipcMain.handle('storage:remove-bookmark', (_e, urlOrId: string) => storage.removeBookmark(urlOrId));
  ipcMain.handle('storage:add-history', (_e, entry) => storage.addHistory(entry));
  ipcMain.handle('storage:clear-history', () => storage.clearHistory());
  ipcMain.handle('storage:remove-history-item', (_e, id: string) => storage.removeHistoryItem(id));
  ipcMain.handle('storage:update-speed-dials', (_e, dials) => storage.updateSpeedDials(dials));
  ipcMain.handle('storage:update-scratchpad', (_e, content: string) => storage.updateScratchpad(content));

  // IPC Handlers — Shield & Security
  ipcMain.handle('shield:get-stats', () => shield.getStats());
  ipcMain.handle('shield:toggle-host', (_e, host: string) => {
    const updated = shield.toggleWhitelistHost(host);
    storage.setWhitelistedHosts(updated.whitelistedHosts);
    return updated;
  });
  ipcMain.handle('security:audit-url', (_e, url: string) => auditSecurityHeaders(url));
  ipcMain.handle('privacy:clear-browsing-data', async () => {
    await mainSession.clearCache();
    await mainSession.clearStorageData({
      storages: ['cookies', 'localstorage', 'indexdb', 'serviceworkers', 'cachestorage'],
    });
    storage.clearHistory();
    return true;
  });

  // IPC Handlers — Downloads
  ipcMain.handle('downloads:get-all', () => downloadsList);
  ipcMain.handle('downloads:show-in-folder', (_e, savePath: string) => {
    if (savePath) shell.showItemInFolder(savePath);
  });
  ipcMain.handle('downloads:open-file', (_e, savePath: string) => {
    if (savePath) shell.openPath(savePath);
  });

  createWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
