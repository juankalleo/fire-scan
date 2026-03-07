import React, { useEffect, useState } from 'react'
import { Search as SearchIcon, Loader2, ChevronRight, X } from 'lucide-react'
import { useSources } from '@/hooks/useSources'
import { apiClient } from '@/utils/apiClient'
import mangalivreLogo from '@/assets/icons/mangalivrelogo.png'
import niaddLogo from '@/assets/icons/niaddlogo.jfif'

interface MangaResult {
  id: string
  title: string
  coverImageUrl?: string
  cover_path?: string
  cover_image_url?: string
  synopsis?: string
  source_id: string
  source_name: string
  author?: string
  status?: string
  rating?: number
  total_chapters?: number
}

type LiveDownloadInfo = {
  status?: string
  progress?: number
  last_stdout?: string
  last_stderr?: string
  error?: string
}

type ViewMode = 'sources' | 'source-list' | 'global-results'

export const SearchPage: React.FC = () => {
  const SOURCE_PAGE_SIZE = 50
  const { sources, isLoading: sourcesLoading } = useSources()
  const [viewMode, setViewMode] = useState<ViewMode>('sources')
  const [selectedSource, setSelectedSource] = useState<string | null>(null)
  const [mangaList, setMangaList] = useState<MangaResult[]>([])
  const [currentPage, setCurrentPage] = useState<number>(1)
  const [totalPages, setTotalPages] = useState<number>(1)
  const [isLoadingManga, setIsLoadingManga] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [isDownloadModalOpen, setIsDownloadModalOpen] = useState(false)
  const [selectedMangaForDownload, setSelectedMangaForDownload] = useState<MangaResult | null>(null)
  const [isLoadingDownloadInfo, setIsLoadingDownloadInfo] = useState(false)
  const [isStartingDownload, setIsStartingDownload] = useState(false)
  const [downloadMode, setDownloadMode] = useState<'all' | 'count'>('all')
  const [availableChapters, setAvailableChapters] = useState<number | null>(null)
  const [chaptersToDownload, setChaptersToDownload] = useState<number>(1)
  const [manualChapterExpression, setManualChapterExpression] = useState<string>('all')
  const [downloadMessage, setDownloadMessage] = useState<string | null>(null)
  const [downloadError, setDownloadError] = useState<string | null>(null)
  const [globalSearchLabel, setGlobalSearchLabel] = useState<string>('')
  const [activeDownloadId, setActiveDownloadId] = useState<string | null>(null)
  const [activeDownloadInfo, setActiveDownloadInfo] = useState<LiveDownloadInfo | null>(null)

  const enabledSources = sources.filter(s => s.enabled)

  const getSourceLogo = (sourceId: string): string | null => {
    const key = sourceId.toLowerCase()
    if (key === 'mangalivre') return mangalivreLogo
    if (key === 'niadd') return niaddLogo
    return null
  }

  const extractErrorMessage = (err: unknown): string => {
    if (typeof err === 'string') return err
    if (err && typeof err === 'object') {
      const maybeMessage = (err as { message?: unknown }).message
      if (typeof maybeMessage === 'string' && maybeMessage.trim().length > 0) {
        return maybeMessage
      }
    }
    return 'Erro desconhecido'
  }

  const fetchSourcePage = async (sourceId: string, page: number, query: string) => {
    setIsLoadingManga(true)
    setError(null)

    try {
      const trimmedQuery = query.trim()
      const res = trimmedQuery.length > 0
        ? await apiClient.searchWeb(sourceId, trimmedQuery, page, SOURCE_PAGE_SIZE)
        : await apiClient.listMangaBySource(sourceId, page, SOURCE_PAGE_SIZE)

      if (res.error) {
        setError(res.message || 'Erro ao buscar fonte')
        setMangaList([])
        setTotalPages(1)
      } else {
        setMangaList(res.results ?? [])
        setCurrentPage(page)
        setTotalPages(Math.max(1, res.total_pages ?? 1))
      }
    } catch (err) {
      const msg = extractErrorMessage(err)
      setError(msg === 'timeout' ? 'A fonte demorou para responder' : msg)
      setMangaList([])
      setTotalPages(1)
    } finally {
      setIsLoadingManga(false)
    }
  }

  const handleSourceClick = async (sourceId: string) => {
    setViewMode('source-list')
    setSelectedSource(sourceId)
    setCurrentPage(1)
    setTotalPages(1)
    setMangaList([])
    setError(null)
    await fetchSourcePage(sourceId, 1, '')
  }

  const handleGlobalSearch = async (rawQuery: string) => {
    const query = rawQuery.trim()
    if (!query) {
      setError('Digite algo para pesquisar')
      return
    }

    if (enabledSources.length === 0) {
      setError('Nenhuma fonte ativa para pesquisar')
      return
    }

    setViewMode('global-results')
    setSelectedSource(null)
    setGlobalSearchLabel(query)
    setIsLoadingManga(true)
    setError(null)
    setMangaList([])

    const dedupe = new Set<string>()
    const merged: MangaResult[] = []
    const failedSources: string[] = []

    try {
      const results = await Promise.allSettled(
        enabledSources.map(async (source) => {
          const response = await apiClient.searchWeb(source.id, query, 1, SOURCE_PAGE_SIZE)
          return { source, response }
        })
      )

      for (const item of results) {
        if (item.status !== 'fulfilled') {
          continue
        }

        const { source, response } = item.value

        if (response?.error) {
          failedSources.push(source.name)
          continue
        }

        const list = Array.isArray(response?.results) ? response.results : []
        for (const manga of list) {
          const sourceId = manga?.source_id || source.id
          const sourceName = manga?.source_name || source.name
          const key = `${sourceId}::${manga.id}`
          if (dedupe.has(key)) {
            continue
          }

          dedupe.add(key)
          merged.push({ ...manga, source_id: sourceId, source_name: sourceName })
        }
      }

      setMangaList(merged)
      setCurrentPage(1)
      setTotalPages(1)

      if (merged.length === 0 && failedSources.length > 0) {
        setError(`Falha ao pesquisar em: ${failedSources.join(', ')}`)
      } else if (failedSources.length > 0) {
        setError(`Algumas fontes falharam: ${failedSources.join(', ')}`)
      }
    } catch (err) {
      const msg = extractErrorMessage(err)
      setError(msg === 'timeout' ? 'As fontes demoraram para responder' : msg)
      setMangaList([])
    } finally {
      setIsLoadingManga(false)
    }
  }

  const goToPreviousPage = async () => {
    if (!selectedSource) return
    if (currentPage <= 1) return
    await fetchSourcePage(selectedSource, currentPage - 1, searchQuery)
  }

  const goToNextPage = async () => {
    if (!selectedSource) return
    if (currentPage >= totalPages) return
    await fetchSourcePage(selectedSource, currentPage + 1, searchQuery)
  }

  const handleShowDetails = async (manga: MangaResult) => {
    try {
      const res = await apiClient.getMangaDetails(manga.source_id, manga.id)
      const full = res.manga
      alert(`Título: ${full.title}\nCapítulos: ${full.total_chapters || 'desconhecido'}\nSinopse: ${full.synopsis || 'N/D'}`)
    } catch (err) {
      console.error('Failed to get details', err)
      alert('Falha ao obter detalhes: ' + (err instanceof Error ? err.message : String(err)))
    }
  }

  const closeDownloadModal = () => {
    setIsDownloadModalOpen(false)
    setSelectedMangaForDownload(null)
    setIsLoadingDownloadInfo(false)
    setIsStartingDownload(false)
    setDownloadMode('all')
    setAvailableChapters(null)
    setChaptersToDownload(1)
    setManualChapterExpression('all')
    setDownloadError(null)
    setActiveDownloadId(null)
    setActiveDownloadInfo(null)
  }

  const handleOpenDownloadModal = async (manga: MangaResult) => {
    setDownloadMessage(null)
    setDownloadError(null)
    setSelectedMangaForDownload(manga)
    setIsDownloadModalOpen(true)
    setIsLoadingDownloadInfo(true)

    const fallbackTotal = manga.total_chapters && manga.total_chapters > 0 ? manga.total_chapters : null
    setAvailableChapters(fallbackTotal)
    setDownloadMode(fallbackTotal ? 'count' : 'all')
    setChaptersToDownload(fallbackTotal ? fallbackTotal : 1)

    try {
      const res = await apiClient.getMangaDetails(manga.source_id, manga.id)
      const detailedTotal = Number(res?.manga?.total_chapters ?? 0)
      if (Number.isFinite(detailedTotal) && detailedTotal > 0) {
        setAvailableChapters(Math.floor(detailedTotal))
        setDownloadMode('count')
        setChaptersToDownload(Math.floor(detailedTotal))
      }
    } catch (err) {
      const msg = extractErrorMessage(err)
      setDownloadError(msg === 'timeout'
        ? 'Não foi possível carregar os detalhes a tempo. Você ainda pode baixar todos os capítulos.'
        : `Detalhes indisponíveis agora (${msg}). Você ainda pode baixar.`)
    } finally {
      setIsLoadingDownloadInfo(false)
    }
  }

  const buildChaptersRequest = (): string => {
    if (downloadMode === 'all') {
      return 'all'
    }

    if (availableChapters && availableChapters > 0) {
      const safeCount = Math.max(1, Math.min(chaptersToDownload, availableChapters))
      return `1-${safeCount}`
    }

    const raw = manualChapterExpression.trim()
    return raw.length > 0 ? raw : 'all'
  }

  const handleConfirmDownload = async () => {
    if (!selectedMangaForDownload) return

    setIsStartingDownload(true)
    setDownloadError(null)

    try {
      const chapters = buildChaptersRequest()
      const started = await apiClient.startDownload(selectedMangaForDownload.id, chapters, 'cbz')
      const startedId = started?.download_id || started?.downloadId

      if (typeof startedId === 'string' && startedId.length > 0) {
        setActiveDownloadId(startedId)
      }

      setActiveDownloadInfo({
        status: started?.status || 'running',
        progress: Number(started?.progress ?? 0),
      })

      const chapterLabel = chapters === 'all' ? 'todos os capítulos' : `capítulos ${chapters}`
      setDownloadMessage(`Download adicionado: ${selectedMangaForDownload.title} (${chapterLabel}).`)
      setIsStartingDownload(false)
    } catch (err) {
      const msg = extractErrorMessage(err)
      setDownloadError(msg === 'timeout' ? 'O download demorou para iniciar. Tente novamente.' : msg)
    } finally {
      setIsStartingDownload(false)
    }
  }

  const handleBackToSources = () => {
    setViewMode('sources')
    setSelectedSource(null)
    setMangaList([])
    setSearchQuery('')
    setCurrentPage(1)
    setTotalPages(1)
    setError(null)
    setGlobalSearchLabel('')
  }

  useEffect(() => {
    if (viewMode !== 'source-list') return
    if (!selectedSource) return

    const timer = setTimeout(async () => {
      await fetchSourcePage(selectedSource, 1, searchQuery)
    }, 450)

    return () => clearTimeout(timer)
  }, [searchQuery, selectedSource, viewMode])

  useEffect(() => {
    if (!isDownloadModalOpen || !activeDownloadId) return

    let mounted = true

    const refreshProgress = async () => {
      try {
        const res = await apiClient.getDownloadProgress(activeDownloadId)
        if (!mounted) return

        const next: LiveDownloadInfo = {
          status: typeof res?.status === 'string' ? res.status : 'running',
          progress: Number(res?.progress ?? 0),
          last_stdout: typeof res?.last_stdout === 'string' ? res.last_stdout : undefined,
          last_stderr: typeof res?.last_stderr === 'string' ? res.last_stderr : undefined,
          error: typeof res?.error === 'string' ? res.error : undefined,
        }

        setActiveDownloadInfo(next)
      } catch {
        // Ignore transient polling errors while downloader is updating state.
      }
    }

    refreshProgress()
    const timer = setInterval(refreshProgress, 1200)

    return () => {
      mounted = false
      clearInterval(timer)
    }
  }, [activeDownloadId, isDownloadModalOpen])

  if (sourcesLoading) {
    return (
      <div className="p-6">
        <h2 className="text-2xl font-bold text-dark-text mb-6">Buscar</h2>
        <div className="flex justify-center py-12">
          <Loader2 className="w-8 h-8 text-brand-primary animate-spin" />
        </div>
      </div>
    )
  }

  if (viewMode === 'sources') {
    return (
      <div className="p-6 space-y-6">
        <div>
          <h2 className="text-2xl font-bold text-dark-text mb-2">Buscar</h2>
          <p className="text-dark-text-secondary text-sm">Pesquise em todas as fontes de uma vez ou clique em uma fonte para navegar.</p>
        </div>

        <form
          onSubmit={(e) => {
            e.preventDefault()
            handleGlobalSearch(searchQuery)
          }}
          className="relative"
        >
          <SearchIcon className="absolute left-3 top-3 w-5 h-5 text-dark-text-secondary" />
          <input
            type="text"
            placeholder="Buscar em todas as fontes (ex.: the supreme)"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-10 pr-28 py-2 rounded-lg bg-dark-surface border border-dark-border text-dark-text placeholder-dark-text-secondary focus:outline-none focus:border-brand-primary"
          />
          <button
            type="submit"
            className="absolute right-1.5 top-1.5 px-4 py-1.5 bg-brand-primary hover:bg-brand-primary/80 text-white rounded-md transition text-sm font-medium"
          >
            Buscar
          </button>
        </form>

        {enabledSources.length === 0 ? (
          <div className="card p-8 text-center">
            <p className="text-dark-text-secondary">Nenhuma fonte ativa</p>
            <p className="text-xs text-dark-text-secondary mt-2">Ative fontes na aba "Fontes"</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
            {enabledSources.map(source => (
              <button
                key={source.id}
                onClick={() => handleSourceClick(source.id)}
                className="card p-4 text-left hover:bg-dark-surface-darker hover:border-brand-primary transition-all group cursor-pointer"
              >
                <div className="flex items-center justify-between">
                  <div className="flex-1">
                    <h3 className="font-semibold text-dark-text group-hover:text-brand-primary transition-colors flex items-center gap-2">
                      {getSourceLogo(source.id) ? (
                        <img
                          src={getSourceLogo(source.id) || ''}
                          alt={`Logo ${source.name}`}
                          className="w-5 h-5 rounded object-cover border border-dark-border"
                          loading="lazy"
                        />
                      ) : (
                        <span className="w-5 h-5 rounded bg-dark-surface flex items-center justify-center text-[10px] text-dark-text-secondary border border-dark-border">
                          {source.name.slice(0, 1).toUpperCase()}
                        </span>
                      )}
                      <span>{source.name}</span>
                    </h3>
                    <p className="text-sm text-dark-text-secondary mt-1">{source.language}</p>
                    {source.description && (
                      <p className="text-xs text-dark-text-secondary mt-2">{source.description}</p>
                    )}
                  </div>
                  <ChevronRight className="w-5 h-5 text-brand-primary group-hover:translate-x-1 transition-transform ml-2 flex-shrink-0" />
                </div>
              </button>
            ))}
          </div>
        )}
      </div>
    )
  }

  const currentSource = sources.find(s => s.id === selectedSource)
  const isGlobalResults = viewMode === 'global-results'

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <button
            onClick={handleBackToSources}
            className="text-brand-primary hover:text-brand-primary/80 text-sm mb-2 transition-colors"
          >
             Voltar
          </button>
          <h2 className="text-2xl font-bold text-dark-text">
            {isGlobalResults ? `Resultados para "${globalSearchLabel}"` : currentSource?.name}
          </h2>
          {!isGlobalResults && (
            <p className="text-dark-text-secondary text-sm">
              Página {currentPage} de {Math.max(totalPages, 1)}
            </p>
          )}
          {isGlobalResults && (
            <p className="text-dark-text-secondary text-sm">
              {mangaList.length} resultado(s) encontrados em {enabledSources.length} fonte(s)
            </p>
          )}
        </div>
      </div>

      {isGlobalResults ? (
        <form
          onSubmit={(e) => {
            e.preventDefault()
            handleGlobalSearch(searchQuery)
          }}
          className="relative"
        >
          <SearchIcon className="absolute left-3 top-3 w-5 h-5 text-dark-text-secondary" />
          <input
            type="text"
            placeholder="Refinar busca global"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-10 pr-28 py-2 rounded-lg bg-dark-surface border border-dark-border text-dark-text placeholder-dark-text-secondary focus:outline-none focus:border-brand-primary"
          />
          <button
            type="submit"
            className="absolute right-1.5 top-1.5 px-4 py-1.5 bg-brand-primary hover:bg-brand-primary/80 text-white rounded-md transition text-sm font-medium"
          >
            Buscar
          </button>
        </form>
      ) : (
        <div className="relative">
          <SearchIcon className="absolute left-3 top-3 w-5 h-5 text-dark-text-secondary" />
          <input
            type="text"
            placeholder="Buscar no site da fonte (ex.: the supreme)..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-10 pr-4 py-2 rounded-lg bg-dark-surface border border-dark-border text-dark-text placeholder-dark-text-secondary focus:outline-none focus:border-brand-primary"
          />
        </div>
      )}

      {error && (
        <div className="bg-red-900/20 border border-red-500/30 rounded-lg p-4 text-red-300">
          <p>Erro: {error}</p>
          <button
            onClick={() => {
              if (selectedSource) {
                fetchSourcePage(selectedSource, currentPage, searchQuery)
              }
            }}
            className="mt-2 px-3 py-1 text-sm bg-red-600/30 hover:bg-red-600/50 rounded transition-colors"
          >
            Tentar novamente
          </button>
        </div>
      )}

      {downloadMessage && (
        <div className="bg-green-900/20 border border-green-500/30 rounded-lg p-4 text-green-200 flex items-start justify-between gap-4">
          <p>{downloadMessage}</p>
          <button
            onClick={() => setDownloadMessage(null)}
            className="text-green-200/80 hover:text-green-100 transition-colors"
            aria-label="Fechar mensagem"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      )}

      {isLoadingManga && (
        <div className="flex justify-center py-12">
          <Loader2 className="w-8 h-8 text-brand-primary animate-spin" />
        </div>
      )}

      {!isLoadingManga && mangaList.length === 0 && !error && (
        <div className="card p-8 text-center space-y-4">
          <p className="text-dark-text-secondary font-medium">
            {isGlobalResults ? 'Nenhum mangá encontrado na busca global' : 'Nenhum mangá encontrado nesta fonte'}
          </p>
          <div className="bg-dark-surface-darker rounded p-3 text-sm text-dark-text-secondary space-y-2">
            <p>
              {isGlobalResults
                ? 'Nenhuma fonte retornou resultados para esse termo no momento.'
                : 'A fonte pode não estar disponível no momento ou sem novos mangás adicionados.'}
            </p>
            <p className="mt-2 text-xs">
              {isGlobalResults ? 'Tente outro termo de pesquisa.' : 'Tente novamente mais tarde ou escolha outra fonte.'}
            </p>
          </div>
          <button
            onClick={handleBackToSources}
            className="inline-block px-4 py-2 bg-brand-primary text-white rounded-lg hover:bg-brand-primary/80 transition-colors text-sm"
          >
             Escolher outra fonte
          </button>
        </div>
      )}

      {!isLoadingManga && mangaList.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {mangaList.map(manga => (
            <div
              key={manga.id}
              className="card overflow-hidden hover:border-brand-primary transition-all group"
            >
              <div className="grid grid-cols-4 gap-4">
                <div className="col-span-1">
                  {(manga.coverImageUrl || manga.cover_path || manga.cover_image_url) ? (
                    <div className="w-full aspect-[3/4] bg-dark-surface-darker overflow-hidden rounded">
                      <img
                        src={manga.coverImageUrl || manga.cover_path || manga.cover_image_url}
                        alt={manga.title}
                        className="w-full h-full object-cover group-hover:scale-105 transition-transform"
                        loading="lazy"
                      />
                    </div>
                  ) : (
                    <div className="w-full aspect-[3/4] bg-dark-surface-darker rounded flex items-center justify-center">
                      <div className="text-center text-dark-text-secondary text-xs">
                        <div></div>
                        <div className="mt-1">Sem capa</div>
                      </div>
                    </div>
                  )}
                </div>

                <div className="col-span-3 flex flex-col">
                  <div className="flex-1">
                    <h3 className="font-semibold text-dark-text line-clamp-2 text-sm">
                      {manga.title}
                    </h3>

                    {isGlobalResults && (
                      <p className="text-xs text-brand-primary mt-1">
                        Fonte: {manga.source_name}
                      </p>
                    )}

                    {manga.author && (
                      <p className="text-xs text-dark-text-secondary mt-1">
                        Autor: {manga.author}
                      </p>
                    )}

                    {manga.status && (
                      <p className="text-xs mt-1">
                        <span className="inline-block px-2 py-0.5 rounded bg-dark-surface-darker text-brand-primary">
                          {manga.status === 'ongoing' ? ' Em Andamento' : ' Completo'}
                        </span>
                      </p>
                    )}

                    {manga.rating && manga.rating > 0 && (
                      <p className="text-xs text-yellow-500 mt-1">
                         {manga.rating.toFixed(1)}
                      </p>
                    )}

                    {manga.total_chapters && (
                      <p className="text-xs text-brand-primary mt-1">
                         {manga.total_chapters} capítulos
                      </p>
                    )}
                  </div>

                  {manga.synopsis && (
                    <p className="text-xs text-dark-text-secondary line-clamp-3 mt-2 py-2 border-t border-dark-border">
                      {manga.synopsis}
                    </p>
                  )}

                  <div className="mt-3 flex gap-2">
                    <button
                      onClick={() => handleShowDetails(manga)}
                      className="flex-1 px-3 py-1.5 bg-dark-surface-darker hover:bg-dark-surface transition-colors text-white text-xs font-medium rounded"
                    >
                      ℹ Detalhes
                    </button>
                    <button
                      onClick={() => handleOpenDownloadModal(manga)}
                      className="px-3 py-1.5 bg-brand-primary hover:bg-brand-primary/80 text-white text-xs font-medium rounded transition-colors"
                    >
                       Download
                    </button>
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {!isLoadingManga && mangaList.length > 0 && !isGlobalResults && (
        <div className="flex items-center justify-center gap-3 mt-6">
          <button
            onClick={goToPreviousPage}
            disabled={currentPage <= 1}
            className="px-4 py-2 bg-dark-surface-darker text-dark-text rounded-lg hover:bg-dark-surface transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Anterior
          </button>
          <span className="text-sm text-dark-text-secondary min-w-[110px] text-center">
            {currentPage} / {Math.max(totalPages, 1)}
          </span>
          <button
            onClick={goToNextPage}
            disabled={currentPage >= totalPages}
            className="px-4 py-2 bg-brand-primary text-white rounded-lg hover:bg-brand-primary/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Próxima
          </button>
        </div>
      )}

      {isDownloadModalOpen && selectedMangaForDownload && (
        <div
          className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4"
          onClick={closeDownloadModal}
        >
          <div
            className="w-full max-w-xl rounded-2xl border border-dark-border bg-dark-surface shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="px-5 py-4 border-b border-dark-border flex items-start justify-between gap-4">
              <div>
                <p className="text-xs uppercase tracking-wider text-brand-primary">Download</p>
                <h3 className="text-lg font-bold text-dark-text leading-tight mt-1">
                  {selectedMangaForDownload.title}
                </h3>
                <p className="text-sm text-dark-text-secondary mt-2">
                  {availableChapters && availableChapters > 0
                    ? `Este título possui ${availableChapters} capítulos disponíveis.`
                    : 'Não foi possível confirmar o total de capítulos agora.'}
                </p>
              </div>
              <button
                onClick={closeDownloadModal}
                className="text-dark-text-secondary hover:text-dark-text transition-colors"
                aria-label="Fechar modal"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="p-5 space-y-4">
              {isLoadingDownloadInfo && (
                <div className="rounded-lg border border-dark-border bg-dark-surface-darker px-3 py-2 flex items-center gap-2 text-sm text-dark-text-secondary">
                  <Loader2 className="w-4 h-4 animate-spin text-brand-primary" />
                  Carregando informações de capítulos...
                </div>
              )}

              {downloadError && (
                <div className="rounded-lg border border-amber-500/30 bg-amber-900/20 px-3 py-2 text-sm text-amber-200">
                  {downloadError}
                </div>
              )}

              {activeDownloadId && activeDownloadInfo && (
                <div className="rounded-lg border border-brand-primary/30 bg-brand-primary/10 px-3 py-3 space-y-2">
                  <div className="flex items-center justify-between gap-3">
                    <p className="text-sm text-dark-text font-medium">
                      Status: {activeDownloadInfo.status || 'running'}
                    </p>
                    <p className="text-xs text-dark-text-secondary">ID: {activeDownloadId.slice(0, 8)}</p>
                  </div>
                  <div className="w-full h-2 rounded-full bg-dark-surface overflow-hidden">
                    <div
                      className="h-full bg-brand-primary transition-all"
                      style={{ width: `${Math.max(0, Math.min(Number(activeDownloadInfo.progress ?? 0), 100))}%` }}
                    />
                  </div>
                  <p className="text-xs text-dark-text-secondary">
                    {Math.max(0, Math.min(Number(activeDownloadInfo.progress ?? 0), 100)).toFixed(0)}%
                  </p>
                  <p className="text-sm text-dark-text-secondary break-words">
                    {activeDownloadInfo.last_stdout || activeDownloadInfo.last_stderr || 'Preparando download...'}
                  </p>
                  {activeDownloadInfo.error && (
                    <p className="text-xs text-red-300">Erro: {activeDownloadInfo.error}</p>
                  )}
                </div>
              )}

              <div className="rounded-xl border border-dark-border bg-dark-surface-darker p-4 space-y-3">
                <div className="flex flex-wrap items-center gap-2">
                  <button
                    onClick={() => setDownloadMode('all')}
                    className={`px-3 py-1.5 text-xs font-semibold rounded-full transition-colors ${
                      downloadMode === 'all'
                        ? 'bg-brand-primary text-white'
                        : 'bg-dark-surface text-dark-text-secondary hover:text-dark-text'
                    }`}
                  >
                    Baixar tudo
                  </button>
                  <button
                    onClick={() => setDownloadMode('count')}
                    className={`px-3 py-1.5 text-xs font-semibold rounded-full transition-colors ${
                      downloadMode === 'count'
                        ? 'bg-brand-primary text-white'
                        : 'bg-dark-surface text-dark-text-secondary hover:text-dark-text'
                    }`}
                  >
                    Escolher quantidade
                  </button>
                </div>

                {downloadMode === 'count' && availableChapters && availableChapters > 0 && (
                  <div className="space-y-3">
                    <label className="text-sm text-dark-text-secondary block">
                      Quantos capítulos baixar agora
                    </label>
                    <div className="flex items-center gap-3">
                      <input
                        type="range"
                        min={1}
                        max={availableChapters}
                        value={Math.max(1, Math.min(chaptersToDownload, availableChapters))}
                        onChange={(e) => setChaptersToDownload(Number(e.target.value))}
                        className="flex-1 accent-brand-primary"
                      />
                      <input
                        type="number"
                        min={1}
                        max={availableChapters}
                        value={Math.max(1, Math.min(chaptersToDownload, availableChapters))}
                        onChange={(e) => setChaptersToDownload(Number(e.target.value) || 1)}
                        className="w-24 px-2 py-1.5 rounded bg-dark-surface border border-dark-border text-dark-text"
                      />
                    </div>
                    <p className="text-xs text-dark-text-secondary">
                      Serão baixados os capítulos 1 até {Math.max(1, Math.min(chaptersToDownload, availableChapters))}.
                    </p>
                  </div>
                )}

                {downloadMode === 'count' && (!availableChapters || availableChapters <= 0) && (
                  <div className="space-y-2">
                    <label className="text-sm text-dark-text-secondary block">
                      Informe os capítulos (ex.: 1-20, 1,3,5)
                    </label>
                    <input
                      type="text"
                      value={manualChapterExpression}
                      onChange={(e) => setManualChapterExpression(e.target.value)}
                      className="w-full px-3 py-2 rounded bg-dark-surface border border-dark-border text-dark-text"
                    />
                  </div>
                )}
              </div>
            </div>

            <div className="px-5 py-4 border-t border-dark-border flex items-center justify-end gap-3">
              <button
                onClick={closeDownloadModal}
                disabled={isStartingDownload}
                className="px-4 py-2 rounded-lg bg-dark-surface-darker border border-dark-border text-dark-text-secondary hover:text-dark-text transition-colors disabled:opacity-60"
              >
                Cancelar
              </button>
              <button
                onClick={handleConfirmDownload}
                disabled={
                  isStartingDownload ||
                  isLoadingDownloadInfo ||
                  (activeDownloadInfo?.status === 'running' || activeDownloadInfo?.status === 'pending')
                }
                className="px-4 py-2 rounded-lg bg-brand-primary text-white hover:bg-brand-primary/80 transition-colors disabled:opacity-60 flex items-center gap-2"
              >
                {isStartingDownload && <Loader2 className="w-4 h-4 animate-spin" />}
                {activeDownloadId ? 'Download em andamento' : 'Baixar capítulos'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
