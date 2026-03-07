import React, { useEffect, useMemo, useState } from 'react'
import { ArrowLeft, BookOpen, Heart, Loader2, RefreshCw, Search } from 'lucide-react'
import { apiClient } from '@/utils/apiClient'
import { convertFileSrc } from '@tauri-apps/api/tauri'
import { EmptyState } from '@/components/common/Loading'

type FavoriteManga = {
  id: string
  title: string
  coverPath?: string
  cover_path?: string
  cover_image_url?: string
  cover?: string
  local_path?: string
  downloaded_chapters?: number
}

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
  const low = input.toLowerCase()
  if (
    low.startsWith('http://') ||
    low.startsWith('https://') ||
    low.startsWith('asset://') ||
    low.startsWith('tauri://') ||
    low.startsWith('data:')
  ) {
    return input
  }
  if (input.startsWith('file://')) {
    const raw = decodeURIComponent(input.replace(/^file:\/\//, '')).replace(/\//g, '\\')
    return convertFileSrc(raw)
  }
  if (/^[a-zA-Z]:\\/.test(input) || input.startsWith('\\\\')) {
    return convertFileSrc(input)
  }
  return input
}

export const FavoritesPage: React.FC = () => {
  const [favorites, setFavorites] = useState<FavoriteManga[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [selectedManga, setSelectedManga] = useState<FavoriteManga | null>(null)
  const [chapters, setChapters] = useState<ChapterItem[]>([])
  const [chapterQuery, setChapterQuery] = useState('')
  const [selectedChapter, setSelectedChapter] = useState<ChapterItem | null>(null)
  const [chapterPages, setChapterPages] = useState<string[]>([])
  const [isLoadingChapters, setIsLoadingChapters] = useState(false)
  const [isLoadingPages, setIsLoadingPages] = useState(false)
  const [readerError, setReaderError] = useState<string | null>(null)

  const loadFavorites = async () => {
    setIsLoading(true)
    setError(null)
    try {
      const res = await apiClient.getFavorites()
      const list = Array.isArray(res?.favorites)
        ? res.favorites.map((i: any) => ({
            ...i,
            cover_path: i.cover_path ?? i.coverPath ?? i.cover_image_url ?? i.cover,
          }))
        : []
      setFavorites(list)
    } catch (e) {
      console.error(e)
      setError('Falha ao carregar favoritos.')
      setFavorites([])
    } finally {
      setIsLoading(false)
    }
  }

  useEffect(() => {
    loadFavorites()
  }, [])

  const getCoverUrl = (m: FavoriteManga): string => {
    const raw = m.cover_path ?? m.coverPath ?? m.cover_image_url ?? m.cover
    return toFileUrl(raw)
  }

  const filteredChapters = useMemo(() => {
    const q = chapterQuery.trim().toLowerCase()
    if (!q) return chapters
    return chapters.filter((c) => {
      const t = (c.title || '').toLowerCase()
      const n = String(c.number ?? '')
      return t.includes(q) || n.includes(q)
    })
  }, [chapters, chapterQuery])

  const onOpenManga = async (m: FavoriteManga) => {
    setSelectedManga(m)
    setSelectedChapter(null)
    setChapterPages([])
    setReaderError(null)
    setIsLoadingChapters(true)
    try {
      const res = await apiClient.listLocalChapters(m.id)
      setChapters((res?.chapters ?? []) as ChapterItem[])
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
      setReaderError('Falha ao abrir capítulo.')
      setChapterPages([])
    } finally {
      setIsLoadingPages(false)
    }
  }

  const onBack = () => {
    setSelectedManga(null)
    setSelectedChapter(null)
    setChapters([])
    setChapterPages([])
    setChapterQuery('')
    setReaderError(null)
  }

  if (selectedManga) {
    return (
      <div className="p-6 space-y-4">
        <div className="flex items-center gap-3 rounded-2xl border border-dark-border/80 bg-dark-surface/85 p-3">
          <button onClick={onBack} className="btn-secondary flex items-center gap-2">
            <ArrowLeft className="w-4 h-4" />
            Voltar
          </button>
          <h2 className="text-xl font-bold tracking-tight text-dark-text">{selectedManga.title}</h2>
        </div>

        {readerError && (
          <div className="bg-red-900/20 border border-red-500/30 rounded-lg p-3 text-red-300 text-sm">
            {readerError}
          </div>
        )}

        <div className="grid grid-cols-1 lg:grid-cols-[320px,1fr] gap-4">
          <div className="card rounded-2xl p-3 space-y-3">
            <div className="relative">
              <Search className="w-4 h-4 absolute left-2 top-1/2 -translate-y-1/2 text-dark-text-secondary" />
              <input
                value={chapterQuery}
                onChange={(e) => setChapterQuery(e.target.value)}
                placeholder="Buscar capítulo"
                className="w-full pl-8 pr-3 py-2.5 bg-dark-surface-alt/90 border border-dark-border rounded-xl text-sm outline-none"
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
                  className={`w-full text-left px-3 py-2.5 rounded-xl border transition ${selectedChapter?.id === ch.id
                    ? 'border-brand-primary/70 bg-brand-primary/10 text-dark-text'
                    : 'border-dark-border bg-dark-surface text-dark-text-secondary hover:text-dark-text'
                  }`}
                >
                  <span className="truncate">{ch.title || `Cap ${ch.number ?? '?'}`}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="card rounded-2xl p-3">
            {!selectedChapter && (
              <div className="h-[68vh] flex items-center justify-center text-center text-dark-text-secondary">
                Selecione um capítulo para ler
              </div>
            )}

            {selectedChapter && isLoadingPages && (
              <div className="h-[68vh] flex items-center justify-center gap-2 text-dark-text-secondary text-sm">
                <Loader2 className="w-5 h-5 animate-spin" />
                Carregando páginas...
              </div>
            )}

            {selectedChapter && !isLoadingPages && (
              <div className="h-[68vh] overflow-y-auto pr-2 space-y-3 bg-black rounded-xl p-2">
                {chapterPages.map((src, idx) => (
                  <div key={`${selectedChapter.id}-${idx}`} className="w-full bg-black flex items-center justify-center rounded-lg">
                    <img
                      src={src}
                      alt={`Página ${idx + 1}`}
                      className="block mx-auto rounded-lg border border-dark-border/50 w-[82%]"
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
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-dark-text flex items-center gap-2">
          <Heart className="h-5 w-5 text-rose-300 fill-current" />
          Favoritos
        </h2>
        <button
          onClick={loadFavorites}
          disabled={isLoading}
          className="p-2.5 rounded-xl bg-dark-surface border border-dark-border text-dark-text hover:border-brand-primary disabled:opacity-50 transition-colors"
          title="Atualizar"
        >
          <RefreshCw className={`w-5 h-5 ${isLoading ? 'animate-spin' : ''}`} />
        </button>
      </div>

      {error && <div className="text-sm text-red-300">{error}</div>}

      {!isLoading && favorites.length === 0 && (
        <EmptyState
          title="Nenhum Favorito"
          description="Marque seus mangás favoritos na biblioteca usando o coração"
        />
      )}

      {isLoading && favorites.length === 0 && (
        <div className="flex justify-center items-center py-12">
          <Loader2 className="w-8 h-8 text-brand-primary animate-spin" />
        </div>
      )}

      {favorites.length > 0 && (
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-4">
          {favorites.map((m) => (
            <button
              key={m.id}
              className="group overflow-hidden rounded-2xl border border-dark-border/80 bg-dark-surface/90 text-left"
              onClick={() => onOpenManga(m)}
            >
              {getCoverUrl(m) ? (
                <div className="w-full aspect-[3/4] bg-dark-surface-darker overflow-hidden">
                  <img
                    src={getCoverUrl(m)}
                    alt={m.title}
                    className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500"
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
              <div className="p-3">
                <h3 className="font-semibold tracking-tight text-dark-text text-sm line-clamp-2">{m.title}</h3>
                <div className="mt-3 btn-secondary w-full rounded-xl text-center">Ler</div>
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
