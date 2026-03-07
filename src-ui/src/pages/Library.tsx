import React, { useEffect, useMemo, useState } from 'react'
import { ArrowLeft, BookOpen, Heart, Loader2, Minus, Plus, RefreshCw, Search, Sparkles } from 'lucide-react'
import { Manga as LibraryManga, useLibrary } from '@/hooks/useLibrary'
import { apiClient } from '@/utils/apiClient'
import { convertFileSrc } from '@tauri-apps/api/tauri'
import { EmptyState } from '@/components/common/Loading'

type SortBy = 'recent' | 'title' | 'read'

type ChapterItem = {
  id: string
  title: string
  number?: number
}

function getInitials(input: string): string {
  return input
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? '')
    .join('')
}

function toFileUrl(input?: string | null): string {
  if (!input) return ''
  if (input.startsWith('http://') || input.startsWith('https://') || input.startsWith('data:')) return input
  if (input.startsWith('file://')) {
    // Strip prefix and convert through tauri so WebView can load local files.
    const raw = input.replace(/^file:\/\//, '').replace(/\//g, '\\')
    return convertFileSrc(raw)
  }
  // Assume absolute local path
  if (/^[a-zA-Z]:\\/.test(input) || input.startsWith('\\\\')) {
    return convertFileSrc(input)
  }
  if (input.startsWith('file://')) return input
  let normalized = input.replace(/\\/g, '/')
  if (!normalized.startsWith('/')) normalized = '/' + normalized
  return `file://${normalized}`
}

export const LibraryPage: React.FC = () => {
  const { manga, isLoading, error, total, hasNextPage, loadMore, refresh } = useLibrary()
  const [sortBy, setSortBy] = useState<SortBy>('recent')
  const [selectedManga, setSelectedManga] = useState<LibraryManga | null>(null)
  const [chapters, setChapters] = useState<ChapterItem[]>([])
  const [chapterQuery, setChapterQuery] = useState('')
  const [selectedChapter, setSelectedChapter] = useState<ChapterItem | null>(null)
  const [chapterPages, setChapterPages] = useState<string[]>([])
  const [isLoadingChapters, setIsLoadingChapters] = useState(false)
  const [isLoadingPages, setIsLoadingPages] = useState(false)
  const [readerError, setReaderError] = useState<string | null>(null)
  const [showChapters, setShowChapters] = useState(true)
  const [readerZoom, setReaderZoom] = useState(82)
  const [favoriteIds, setFavoriteIds] = useState<Set<string>>(new Set())
  const [favoriteBusy, setFavoriteBusy] = useState<Set<string>>(new Set())

  useEffect(() => {
    let mounted = true
    ;(async () => {
      try {
        const res = await apiClient.getFavorites()
        const list = Array.isArray(res?.favorites) ? res.favorites : []
        if (!mounted) return
        setFavoriteIds(new Set(list.map((f: any) => String(f.id))))
      } catch (e) {
        console.debug('Failed to load favorites', e)
      }
    })()

    return () => {
      mounted = false
    }
  }, [])

  const toggleFavorite = async (mangaId: string) => {
    if (favoriteBusy.has(mangaId)) return

    setFavoriteBusy(prev => new Set(prev).add(mangaId))
    try {
      if (favoriteIds.has(mangaId)) {
        await apiClient.removeFromFavorites(mangaId)
        setFavoriteIds(prev => {
          const next = new Set(prev)
          next.delete(mangaId)
          return next
        })
      } else {
        await apiClient.addToFavorites(mangaId)
        setFavoriteIds(prev => new Set(prev).add(mangaId))
      }
    } catch (e) {
      console.error('Failed to toggle favorite', e)
    } finally {
      setFavoriteBusy(prev => {
        const next = new Set(prev)
        next.delete(mangaId)
        return next
      })
    }
  }

  const sortedManga = [...manga].sort((a, b) => {
    switch (sortBy) {
      case 'title':
        return a.title.localeCompare(b.title, 'pt-BR')
      case 'read':
        return (b.chapters_count || 0) - (a.chapters_count || 0)
      case 'recent':
      default:
        return new Date(b.last_read_date || 0).getTime() - new Date(a.last_read_date || 0).getTime()
    }
  })

  const filteredChapters = useMemo(() => {
    const q = chapterQuery.trim().toLowerCase()
    if (!q) return chapters
    return chapters.filter((c) => {
      const t = (c.title || '').toLowerCase()
      const n = String(c.number ?? '')
      return t.includes(q) || n.includes(q)
    })
  }, [chapters, chapterQuery])

  const onOpenManga = async (m: LibraryManga) => {
    setSelectedManga(m)
    setSelectedChapter(null)
    setChapterPages([])
    setReaderError(null)
    setReaderZoom(82)
    setIsLoadingChapters(true)
    try {
      const res = await apiClient.listLocalChapters(m.id)
      const items = (res?.chapters ?? []) as ChapterItem[]
      setChapters(items)
    } catch (e) {
      console.error(e)
      setReaderError('Falha ao listar capítulos locais.')
      setChapters([])
    } finally {
      setIsLoadingChapters(false)
    }
  }

  const onOpenChapter = async (ch: ChapterItem) => {
    if (!selectedManga) return
    setSelectedChapter(ch)
    setReaderError(null)
    setIsLoadingPages(true)
    try {
      const res = await apiClient.getChapterPages(selectedManga.id, ch.id)
      const pages = (res?.pages ?? []) as string[]
      setChapterPages(pages.map((p) => toFileUrl(p)))
      await apiClient.markChapterRead(selectedManga.id, ch.id, pages.length, pages.length)
    } catch (e) {
      console.error(e)
      setReaderError('Falha ao abrir capítulo. Verifique se o CBZ existe.')
      setChapterPages([])
    } finally {
      setIsLoadingPages(false)
    }
  }

  const onBackToLibrary = () => {
    setSelectedManga(null)
    setSelectedChapter(null)
    setChapters([])
    setChapterPages([])
    setChapterQuery('')
    setReaderError(null)
    setReaderZoom(82)
  }

  if (selectedManga) {
    return (
      <div className="relative p-6 space-y-4">
        <div className="pointer-events-none absolute inset-0 opacity-60">
          <div className="absolute -top-28 right-10 h-64 w-64 rounded-full bg-brand-primary/15 blur-3xl" />
          <div className="absolute bottom-10 left-1/3 h-40 w-40 rounded-full bg-sky-400/10 blur-3xl" />
        </div>

        <div className="relative z-10 flex flex-wrap items-center gap-3 rounded-2xl border border-dark-border/80 bg-dark-surface/85 p-3 shadow-[0_8px_24px_rgba(0,0,0,0.35)] backdrop-blur-sm">
          <button
            onClick={onBackToLibrary}
            className="btn-secondary flex items-center gap-2"
          >
            <ArrowLeft className="w-4 h-4" />
            Voltar
          </button>
          <button
            onClick={() => setShowChapters((v) => !v)}
            className="btn-secondary"
          >
            {showChapters ? 'Esconder capítulos' : 'Mostrar capítulos'}
          </button>
          <h2 className="text-xl font-bold tracking-tight text-dark-text">{selectedManga.title}</h2>

          <div className="ml-auto flex items-center gap-2 rounded-xl border border-dark-border bg-dark-surface-alt/60 px-2 py-1.5">
            <button
              onClick={() => setReaderZoom((z) => Math.max(60, z - 5))}
              className="btn-ghost p-1"
              title="Diminuir zoom"
            >
              <Minus className="w-4 h-4" />
            </button>
            <input
              type="range"
              min={60}
              max={100}
              step={1}
              value={readerZoom}
              onChange={(e) => setReaderZoom(Number(e.target.value))}
              className="w-24"
              title="Zoom do capítulo"
            />
            <button
              onClick={() => setReaderZoom((z) => Math.min(100, z + 5))}
              className="btn-ghost p-1"
              title="Aumentar zoom"
            >
              <Plus className="w-4 h-4" />
            </button>
            <span className="text-xs text-dark-text-secondary w-10 text-right">{readerZoom}%</span>
          </div>

          {selectedManga.chapters_count ? (
            <span className="inline-flex items-center rounded-full border border-brand-primary/30 bg-brand-primary/10 px-3 py-1 text-xs font-semibold text-brand-primary">
              {selectedManga.chapters_count} capítulos
            </span>
          ) : null}
        </div>

        {readerError && (
          <div className="bg-red-900/20 border border-red-500/30 rounded-lg p-3 text-red-300 text-sm">
            {readerError}
          </div>
        )}

        <div className={`relative z-10 grid grid-cols-1 ${showChapters ? 'lg:grid-cols-[320px,1fr]' : ''} gap-4`}>
          {showChapters && (
          <div className="card rounded-2xl p-3 space-y-3 shadow-[0_10px_28px_rgba(0,0,0,0.28)]">
            <div className="relative">
              <Search className="w-4 h-4 absolute left-2 top-1/2 -translate-y-1/2 text-dark-text-secondary" />
              <input
                value={chapterQuery}
                onChange={(e) => setChapterQuery(e.target.value)}
                placeholder="Buscar cap (ex: 12)"
                className="w-full pl-8 pr-3 py-2.5 bg-dark-surface-alt/90 border border-dark-border rounded-xl text-sm outline-none focus:border-brand-primary/70"
              />
            </div>

            <div className="max-h-[68vh] overflow-y-auto space-y-2 pr-1">
              {isLoadingChapters && (
                <div className="flex items-center gap-2 text-sm text-dark-text-secondary">
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Carregando capítulos...
                </div>
              )}

              {!isLoadingChapters && filteredChapters.length === 0 && (
                <div className="text-sm text-dark-text-secondary">Nenhum capítulo local encontrado.</div>
              )}

              {filteredChapters.map((ch) => (
                <button
                  key={ch.id}
                  onClick={() => onOpenChapter(ch)}
                  className={`w-full text-left px-3 py-2.5 rounded-xl border transition-all duration-200 ${selectedChapter?.id === ch.id
                    ? 'border-brand-primary/70 bg-gradient-to-r from-brand-primary/20 to-brand-primary/5 text-dark-text shadow-[0_6px_14px_rgba(58,123,255,0.2)]'
                    : 'border-dark-border bg-dark-surface hover:border-brand-primary/60 hover:bg-dark-surface-alt text-dark-text-secondary hover:text-dark-text'
                  }`}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate">{ch.title || `Cap ${ch.number ?? '?'}`}</span>
                    <span className="text-[11px] text-dark-text-secondary">Ler</span>
                  </div>
                </button>
              ))}
            </div>
          </div>
          )}

          <div className="card rounded-2xl p-3 shadow-[0_10px_28px_rgba(0,0,0,0.28)]">
            {!selectedChapter && (
              <div className={`${showChapters ? 'h-[68vh]' : 'h-[82vh]'} flex items-center justify-center`}>
                <div className="text-center max-w-sm px-4">
                  <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl border border-dark-border bg-dark-surface-alt text-brand-primary">
                    <BookOpen className="h-7 w-7" />
                  </div>
                  <p className="text-2xl font-semibold text-dark-text">Pronto para ler?</p>
                  <p className="mt-2 text-sm text-dark-text-secondary">Selecione um capítulo na lista para começar sua leitura.</p>
                </div>
              </div>
            )}

            {selectedChapter && isLoadingPages && (
              <div className={`${showChapters ? 'h-[68vh]' : 'h-[82vh]'} flex items-center justify-center gap-2 text-dark-text-secondary text-sm`}>
                <Loader2 className="w-5 h-5 animate-spin" />
                Carregando páginas do CBZ...
              </div>
            )}

            {selectedChapter && !isLoadingPages && (
              <div className={`${showChapters ? 'h-[68vh]' : 'h-[82vh]'} overflow-y-auto pr-2 space-y-3 bg-black rounded-xl p-2`}>
                {chapterPages.length === 0 && (
                  <div className="text-sm text-dark-text-secondary">Capítulo sem páginas.</div>
                )}
                {chapterPages.map((src, idx) => (
                  <div key={`${selectedChapter.id}-${idx}`} className="w-full bg-black flex items-center justify-center rounded-lg">
                    <img
                      src={src}
                      alt={`Página ${idx + 1}`}
                      className="block mx-auto rounded-lg border border-dark-border/50"
                      style={{ width: `${readerZoom}%`, height: 'auto' }}
                      loading="lazy"
                    />
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="relative p-6 space-y-6">
      <div className="pointer-events-none absolute inset-0 opacity-70">
        <div className="absolute -top-24 -right-8 h-64 w-64 rounded-full bg-brand-primary/15 blur-3xl" />
        <div className="absolute top-1/3 left-8 h-48 w-48 rounded-full bg-cyan-400/10 blur-3xl" />
      </div>

      <div className="relative z-10 flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-dark-border/80 bg-dark-surface/85 p-4 shadow-[0_10px_28px_rgba(0,0,0,0.3)] backdrop-blur-sm">
        <div className="flex-1">
          <h2 className="text-2xl font-bold tracking-tight text-dark-text flex items-center gap-2">
            <Sparkles className="w-5 h-5 text-brand-primary" />
            Biblioteca {total > 0 && <span className="text-dark-text-secondary text-lg">({total})</span>}
          </h2>
          <p className="text-xs text-dark-text-secondary mt-1">Sua coleção organizada para leitura rápida</p>
        </div>
        <div className="flex gap-3">
          <button
            onClick={refresh}
            disabled={isLoading}
            className="p-2.5 rounded-xl bg-dark-surface border border-dark-border text-dark-text hover:border-brand-primary disabled:opacity-50 transition-colors"
            title="Atualizar"
          >
            <RefreshCw className={`w-5 h-5 ${isLoading ? 'animate-spin' : ''}`} />
          </button>
          <select
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as SortBy)}
            className="px-3 py-2.5 rounded-xl bg-dark-surface border border-dark-border text-dark-text text-sm hover:border-brand-primary transition-colors"
          >
            <option value="recent">Recentes</option>
            <option value="title">Título (A-Z)</option>
            <option value="read">Mais lido</option>
          </select>
        </div>
      </div>

      {/* Error State */}
      {error && (
        <div className="bg-red-900/20 border border-red-500/30 rounded-lg p-4 text-red-300">
          <p>Erro ao carregar biblioteca: {error}</p>
          <button
            onClick={refresh}
            className="mt-2 px-3 py-1 text-sm bg-red-600/30 hover:bg-red-600/50 rounded transition-colors"
          >
            Tentar novamente
          </button>
        </div>
      )}

      {/* Empty State */}
      {!isLoading && total === 0 && !error && (
        <EmptyState
          title="Biblioteca Vazia"
          description="Comece buscando mangás para adicionar à sua biblioteca"
        />
      )}

      {/* Loading State (Initial) */}
      {isLoading && manga.length === 0 && (
        <div className="flex justify-center items-center py-12">
          <Loader2 className="w-8 h-8 text-brand-primary animate-spin" />
        </div>
      )}

      {/* Manga Grid */}
      {sortedManga.length > 0 && (
        <div className="relative z-10">
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-4">
            {sortedManga.map(m => (
              <div
                key={m.id}
                className="library-card-enter group relative overflow-hidden rounded-2xl border border-dark-border/80 bg-dark-surface/90 shadow-[0_10px_24px_rgba(0,0,0,0.28)] transition-all duration-300 hover:-translate-y-1 hover:border-brand-primary/60 hover:shadow-[0_16px_32px_rgba(0,0,0,0.38)] cursor-pointer"
                onClick={() => onOpenManga(m)}
              >
                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    toggleFavorite(m.id)
                  }}
                  disabled={favoriteBusy.has(m.id)}
                  className={`absolute right-2 top-2 z-20 rounded-lg border px-2 py-1 transition ${favoriteIds.has(m.id)
                    ? 'border-rose-400/60 bg-rose-500/20 text-rose-300'
                    : 'border-dark-border/80 bg-black/35 text-dark-text-secondary hover:text-rose-300 hover:border-rose-400/50'
                  } disabled:opacity-50`}
                  title={favoriteIds.has(m.id) ? 'Remover dos favoritos' : 'Adicionar aos favoritos'}
                >
                  <Heart className={`h-4 w-4 ${favoriteIds.has(m.id) ? 'fill-current' : ''}`} />
                </button>

                <div className="absolute inset-0 bg-gradient-to-t from-black/35 via-transparent to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />

                {m.coverImageUrl ? (
                  <div className="w-full aspect-[3/4] bg-dark-surface-darker overflow-hidden">
                    <img
                      src={m.coverImageUrl}
                      alt={m.title}
                      className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500"
                      onError={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none' }}
                      loading="lazy"
                    />
                  </div>
                ) : (
                  <div className="w-full aspect-[3/4] grid place-items-center bg-gradient-to-br from-dark-surface-alt via-dark-surface to-dark-bg">
                    <div className="flex h-16 w-16 items-center justify-center rounded-2xl border border-dark-border bg-dark-surface/70 text-lg font-bold text-brand-primary">
                      {getInitials(m.title || 'M')}
                    </div>
                  </div>
                )}

                <div className="p-3 relative">
                  <h3 className="font-semibold tracking-tight text-dark-text text-sm line-clamp-2">{m.title}</h3>
                  {m.author && (
                    <p className="text-xs text-dark-text-secondary truncate mt-1">
                      {m.author}
                    </p>
                  )}
                  {m.chapters_count && (
                    <p className="inline-flex rounded-full border border-brand-primary/30 bg-brand-primary/10 px-2 py-1 text-[11px] font-semibold text-brand-primary mt-2">
                      {m.chapters_count} capítulos
                    </p>
                  )}
                  <button
                    onClick={(e) => { e.stopPropagation(); onOpenManga(m) }}
                    className="mt-3 btn-secondary w-full rounded-xl"
                  >
                    Abrir
                  </button>
                </div>
              </div>
            ))}
          </div>

          {/* Load More */}
          {hasNextPage && (
            <div className="flex justify-center pt-6">
              <button
                onClick={loadMore}
                disabled={isLoading}
                className="px-4 py-2 bg-brand-primary text-white rounded-lg hover:bg-brand-primary/80 disabled:opacity-50 transition-colors"
              >
                {isLoading ? (
                  <>
                    <Loader2 className="w-4 h-4 inline mr-2 animate-spin" />
                    Carregando...
                  </>
                ) : (
                  'Carregar mais'
                )}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
