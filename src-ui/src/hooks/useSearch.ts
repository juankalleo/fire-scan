import { useState, useCallback } from 'react';
import { apiClient } from '@/utils/apiClient';

export interface SearchResult {
  id: string;
  title: string;
  description?: string;
  coverImageUrl?: string;
  source_id: string;
  author?: string;
  status?: string;
}

export interface UseSearchReturn {
  results: SearchResult[];
  isLoading: boolean;
  error: string | null;
  query: string;
  total: number;
  search: (query: string, sources?: string[]) => Promise<void>;
  clearResults: () => void;
}

export function useSearch(): UseSearchReturn {
  const [results, setResults] = useState<SearchResult[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [total, setTotal] = useState(0);

  const search = useCallback(async (searchQuery: string, sources?: string[]) => {
    if (!searchQuery.trim()) {
      setResults([]);
      setQuery('');
      setTotal(0);
      return;
    }

    setIsLoading(true);
    setError(null);
    setQuery(searchQuery);

    try {
      const response = await apiClient.searchManga(searchQuery, sources);
      setResults(response.results || []);
      setTotal(response.total || 0);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Search failed';
      setError(errorMessage);
      setResults([]);
      setTotal(0);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const clearResults = useCallback(() => {
    setResults([]);
    setQuery('');
    setError(null);
    setTotal(0);
  }, []);

  return {
    results,
    isLoading,
    error,
    query,
    total,
    search,
    clearResults,
  };
}
