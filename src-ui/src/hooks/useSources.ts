import { useState, useEffect } from 'react';
import { apiClient } from '@/utils/apiClient';

export interface Source {
  id: string;
  name: string;
  url: string;
  language: string;
  region: string;
  enabled: boolean;
  priority: number;
  description?: string;
}

export interface UseSourcesReturn {
  sources: Source[];
  isLoading: boolean;
  error: string | null;
  total: number;
  enabled: number;
  refresh: () => Promise<void>;
  toggleSource: (sourceId: string, enabled: boolean) => Promise<void>;
}

export function useSources(): UseSourcesReturn {
  const [sources, setSources] = useState<Source[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [total, setTotal] = useState(0);
  const [enabled, setEnabled] = useState(0);

  const loadSources = async () => {
    setIsLoading(true);
    setError(null);

    try {
      const response = await apiClient.getAvailableSources();
      setSources(response.sources || []);
      setTotal(response.total || 0);
      setEnabled(response.enabled || 0);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to load sources';
      setError(errorMessage);
      setSources([]);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadSources();
  }, []);

  const toggleSource = async (sourceId: string, enabledState: boolean) => {
    try {
      await apiClient.updateSourceSettings(sourceId, enabledState);
      // Update local state
      setSources(prev =>
        prev.map(s =>
          s.id === sourceId ? { ...s, enabled: enabledState } : s
        )
      );
      setEnabled(prev => enabledState ? prev + 1 : prev - 1);
    } catch (err) {
      console.error('Failed to update source:', err);
    }
  };

  return {
    sources,
    isLoading,
    error,
    total,
    enabled,
    refresh: loadSources,
    toggleSource,
  };
}
