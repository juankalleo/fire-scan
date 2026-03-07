import { invoke } from '@tauri-apps/api/tauri'

/**
 * Type-safe wrapper for Tauri IPC commands
 */
export const api = {
  // Library commands
  getLibrary: async (page: number = 1, pageSize: number = 10) =>
    api._invokeWithTimeout('get_library', { page, pageSize }, 12000),

  // Search commands
  searchManga: async (query: string, sources?: string[]) =>
    invoke<any>('search_manga', { query, sources }),

  // Helper: invoke with timeout in ms
  _invokeWithTimeout: async (cmd: string, payload: any, timeoutMs: number = 25000) => {
    const invokePromise = invoke<any>(cmd, payload)
    const timeoutPromise = new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), timeoutMs))
    return Promise.race([invokePromise, timeoutPromise]) as Promise<any>
  },

  listMangaBySource: async (sourceId: string, page: number = 1, pageSize: number = 10) =>
    api._invokeWithTimeout('list_manga_by_source', { sourceId, page, pageSize }, 30000),

  searchWeb: async (sourceId: string, query?: string, page: number = 1, pageSize: number = 10) =>
    api._invokeWithTimeout('search_web', { sourceId, query: query || '', page, pageSize }, 30000),

  // Details
  getMangaDetails: async (sourceId: string, mangaId: string) =>
    api._invokeWithTimeout('get_manga_details', { sourceId, mangaId }, 15000),

  // Download commands
  startDownload: async (url: string, chapters: string | 'all', format: string = 'cbz') =>
    invoke<any>('start_download', { url, chapters, format }),

  getDownloadProgress: async (downloadId: string) =>
    invoke<any>('get_download_progress', { downloadId }),

  listDownloads: async () =>
    invoke<any>('list_downloads'),
  listDownloadedItems: async () =>
    invoke<any>('list_downloaded_items'),
  removeDownloadedManga: async (mangaId: string) =>
    invoke<any>('remove_downloaded_manga', { mangaId }),
  removeDownload: async (downloadId: string) =>
    invoke<any>('remove_download', { downloadId }),

  // Reader commands
  listLocalChapters: async (mangaId: string) =>
    invoke<any>('list_local_chapters', { mangaId }),

  getChapterPages: async (mangaId: string, chapterId: string) =>
    invoke<any>('get_chapter_pages', { mangaId, chapterId }),

  markChapterRead: async (mangaId: string, chapterId: string, currentPage: number, totalPages: number) =>
    invoke<any>('mark_chapter_read', { mangaId, chapterId, currentPage, totalPages }),

  // Favorites commands
  addToFavorites: async (mangaId: string) =>
    invoke<any>('add_to_favorites', { mangaId }),

  removeFromFavorites: async (mangaId: string) =>
    invoke<any>('remove_from_favorites', { mangaId }),

  getFavorites: async () =>
    invoke<any>('get_favorites'),

  // Sources commands
  getAvailableSources: async () =>
    invoke<any>('get_available_sources'),

  updateSourceSettings: async (sourceId: string, enabled: boolean) =>
    invoke<any>('update_source_settings', { source_id: sourceId, enabled }),

  // Settings commands
  getSettings: async () =>
    invoke<any>('get_settings'),

  updateSettings: async (settings: any) =>
    invoke<any>('update_settings', { settings }),

  // Library path commands
  getLibraryPath: async () => invoke<any>('get_library_path_cmd'),
  setLibraryPath: async (path: string) => {
    try {
      // Tauri command args are typically mapped from camelCase to snake_case.
      return await invoke<any>('set_library_path_cmd', { newPath: path })
    } catch {
      // Backward-compat fallback for handlers expecting explicit snake_case key.
      return await invoke<any>('set_library_path_cmd', { new_path: path })
    }
  },

  // Admin commands
  populateTestData: async () =>
    invoke<any>('populate_test_data'),
}

export type ApiCommandError = {
  message: string
  code?: string
}

/**
 * Higher-level API client with helpers
 */
export const apiClient = {
  searchManga: api.searchManga,
  listMangaBySource: api.listMangaBySource,
  searchWeb: api.searchWeb,
  startDownload: api.startDownload,
  getDownloadProgress: api.getDownloadProgress,
  listDownloads: api.listDownloads,
  getLibrary: api.getLibrary,
  getAvailableSources: api.getAvailableSources,
  updateSourceSettings: api.updateSourceSettings,
  getChapterPages: api.getChapterPages,
  listLocalChapters: api.listLocalChapters,
  markChapterRead: api.markChapterRead,
  getMangaDetails: api.getMangaDetails,
  addToFavorites: api.addToFavorites,
  removeFromFavorites: api.removeFromFavorites,
  getFavorites: api.getFavorites,
  populateTestData: api.populateTestData,
  listDownloadedItems: api.listDownloadedItems,
  removeDownloadedManga: api.removeDownloadedManga,
}


// Force rebuild cache bust v1

