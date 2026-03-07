import { useState, useEffect, useCallback } from 'react';
import { apiClient } from '@/utils/apiClient';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { convertFileSrc } from '@tauri-apps/api/tauri';

function toFileUrl(p?: string | null) {
  if (!p) return undefined;
  try {
    const low = p.toLowerCase();
    if (low.startsWith('http://') || low.startsWith('https://') || low.startsWith('asset://') || low.startsWith('tauri://') || low.startsWith('data:')) {
      return p;
    }
    // Required for local filesystem paths in Tauri webview.
    return convertFileSrc(p);
  } catch {
    // Fallback to file:// path in case convertFileSrc is unavailable.
    const normalized = p.replace(/\\/g, '/');
    if (normalized.startsWith('file:')) return normalized;
    return normalized.startsWith('/') ? `file://${normalized}` : `file:///${normalized}`;
  }
}

export interface Manga {
  id: string;
  title: string;
  description?: string;
  coverImageUrl?: string;
  author?: string;
  status?: string;
  source_id: string;
  chapters_count?: number;
  last_read_date?: string;
}

export interface UseLibraryReturn {
  manga: Manga[];
  isLoading: boolean;
  error: string | null;
  total: number;
  page: number;
  pageSize: number;
  hasNextPage: boolean;
  loadMore: () => Promise<void>;
  refresh: () => Promise<void>;
}

export function useLibrary(pageSize: number = 20): UseLibraryReturn {
  const [manga, setManga] = useState<Manga[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [hasNextPage, setHasNextPage] = useState(false);

  const loadPage = useCallback(async (pageNum: number) => {
    setIsLoading(true);
    setError(null);

    try {
      const response = await apiClient.getLibrary(pageNum, pageSize);
      // Backend may return { items: [...], totalPages } or { manga: [...], total }
      const items = (response.items ?? response.manga ?? []) as any[];
      // Normalize fields expected by UI
      const mapped = items.map(i => ({
          id: i.id,
          title: (i.title || i.id || '').replace(/_/g, ' ').replace(/\s+html$/i, '').replace(/\s+/g, ' ').trim(),
        description: i.synopsis ?? i.description,
        coverImageUrl: toFileUrl(i.coverPath ?? i.cover_path ?? i.cover),
        author: i.author,
        status: i.status,
        source_id: i.source_id ?? i.sourceId ?? i.source,
        chapters_count:
          i.downloadedChapters ??
          i.downloaded_chapters ??
          i.totalChapters ??
          i.total_chapters ??
          i.chapters_count,
        last_read_date: i.lastUpdated ?? i.last_read_date ?? i.last_updated,
      }));

      if (pageNum === 1) {
        setManga(mapped);
      } else {
        setManga(prev => [...prev, ...mapped]);
      }

      // Try to compute total count (best-effort)
      const totalFromResp = response.total;
      const possibleTotal = typeof totalFromResp === 'number' && totalFromResp >= 0
        ? totalFromResp
        : (mapped.length || 0);
      setTotal(possibleTotal);
      setPage(pageNum);
      setHasNextPage((mapped.length || 0) === pageSize);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to load library';
      setError(errorMessage);
      if (pageNum === 1) {
        setManga([]);
      }
    } finally {
      setIsLoading(false);
    }
  }, [pageSize]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    let mounted = true;

    (async () => {
      await loadPage(1);

      // Listen for backend 'library-updated' events and refresh UI
      try {
        unlisten = await listen('library-updated', (e) => {
          const p: any = (e as any).payload ?? {};
          if (!mounted) return;
          // If payload provides full items, replace state
          const items = p.items ?? p.items_json ?? p;
          if (Array.isArray(items)) {
            const mapped = (items as any[]).map(i => ({
              id: i.id,
              title: i.title,
              description: i.synopsis ?? i.description,
              coverImageUrl: toFileUrl(i.coverPath ?? i.cover_path ?? i.cover),
              author: i.author,
              status: i.status,
              source_id: i.source_id ?? i.sourceId ?? i.source,
              chapters_count:
                i.downloadedChapters ??
                i.downloaded_chapters ??
                i.totalChapters ??
                i.total_chapters ??
                i.chapters_count,
              last_read_date: i.lastUpdated ?? i.last_read_date ?? i.last_updated,
            }));
            setManga(mapped);
            setTotal(mapped.length);
          } else {
            // fallback: trigger full reload
            loadPage(1);
          }
        });
      } catch (e) {
        console.debug('Failed to subscribe to library-updated event', e);
      }
    })();

    return () => {
      mounted = false;
      if (unlisten) unlisten();
    };
  }, [loadPage]);

  const loadMore = useCallback(async () => {
    if (!isLoading && hasNextPage) {
      await loadPage(page + 1);
    }
  }, [page, isLoading, hasNextPage, loadPage]);

  const refresh = useCallback(async () => {
    await loadPage(1);
  }, [loadPage]);

  return {
    manga,
    isLoading,
    error,
    total,
    page,
    pageSize,
    hasNextPage,
    loadMore,
    refresh,
  };
}
