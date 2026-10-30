import { contextBridge, ipcRenderer } from 'electron';

contextBridge.exposeInMainWorld('orakle', {
  window: {
    minimize: () => ipcRenderer.invoke('window:minimize'),
    maximize: () => ipcRenderer.invoke('window:maximize'),
    close: () => ipcRenderer.invoke('window:close'),
    isMaximized: () => ipcRenderer.invoke('window:is-maximized'),
    onMaximizedChange: (cb: (maximized: boolean) => void) => {
      const listener = (_e: unknown, val: boolean) => cb(val);
      ipcRenderer.on('window:maximized-change', listener);
      return () => ipcRenderer.removeListener('window:maximized-change', listener);
    },
  },
  storage: {
    getAll: () => ipcRenderer.invoke('storage:get-all'),
    updateSettings: (patch: Record<string, unknown>) => ipcRenderer.invoke('storage:update-settings', patch),
    addBookmark: (bm: { title: string; url: string; category: string }) =>
      ipcRenderer.invoke('storage:add-bookmark', bm),
    removeBookmark: (urlOrId: string) => ipcRenderer.invoke('storage:remove-bookmark', urlOrId),
    addHistory: (entry: { title: string; url: string }) => ipcRenderer.invoke('storage:add-history', entry),
    clearHistory: () => ipcRenderer.invoke('storage:clear-history'),
    removeHistoryItem: (id: string) => ipcRenderer.invoke('storage:remove-history-item', id),
    updateSpeedDials: (dials: unknown[]) => ipcRenderer.invoke('storage:update-speed-dials', dials),
    updateScratchpad: (content: string) => ipcRenderer.invoke('storage:update-scratchpad', content),
    saveSpaces: (spaces: unknown[]) => ipcRenderer.invoke('storage:save-spaces', spaces),
    saveSession: (session: { name: string; emoji: string; tabs: { title: string; url: string }[] }) =>
      ipcRenderer.invoke('storage:save-session', session),
    deleteSession: (id: string) => ipcRenderer.invoke('storage:delete-session', id),
  },
  shield: {
    getStats: () => ipcRenderer.invoke('shield:get-stats'),
    toggleHost: (host: string) => ipcRenderer.invoke('shield:toggle-host', host),
    onUpdated: (cb: (stats: unknown) => void) => {
      const listener = (_e: unknown, stats: unknown) => cb(stats);
      ipcRenderer.on('shield:updated', listener);
      return () => ipcRenderer.removeListener('shield:updated', listener);
    },
  },
  security: {
    auditUrl: (url: string) => ipcRenderer.invoke('security:audit-url', url),
    clearBrowsingData: () => ipcRenderer.invoke('privacy:clear-browsing-data'),
    clearData: (types: string[]) => ipcRenderer.invoke('privacy:clear-data', types),
  },
  downloads: {
    getAll: () => ipcRenderer.invoke('downloads:get-all'),
    showInFolder: (savePath: string) => ipcRenderer.invoke('downloads:show-in-folder', savePath),
    openFile: (savePath: string) => ipcRenderer.invoke('downloads:open-file', savePath),
    onUpdated: (cb: (list: unknown[]) => void) => {
      const listener = (_e: unknown, list: unknown[]) => cb(list);
      ipcRenderer.on('downloads:updated', listener);
      return () => ipcRenderer.removeListener('downloads:updated', listener);
    },
  },
  reader: {
    extract: (url: string) => ipcRenderer.invoke('reader:extract', url),
    summarize: (text: string) => ipcRenderer.invoke('reader:summarize', text),
  },
  network: {
    probe: (url: string) => ipcRenderer.invoke('network:probe', url),
  },
  privacy: {
    cookies: (partition?: string) => ipcRenderer.invoke('privacy:cookies', partition),
    clearCookies: (domain: string, partition?: string) =>
      ipcRenderer.invoke('privacy:clear-cookies', domain, partition),
  },
  tabs: {
    onOpenFromMain: (
      cb: (payload: { url: string; background?: boolean; isPrivate?: boolean }) => void
    ) => {
      const listener = (
        _e: unknown,
        payload: { url: string; background?: boolean; isPrivate?: boolean }
      ) => cb(payload);
      ipcRenderer.on('tabs:open-from-main', listener);
      return () => ipcRenderer.removeListener('tabs:open-from-main', listener);
    },
  },
});
