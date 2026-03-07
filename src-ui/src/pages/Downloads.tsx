import React, { useEffect, useState } from 'react'
import { apiClient } from '@/utils/apiClient'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { convertFileSrc } from '@tauri-apps/api/tauri'
import { MoreHorizontal, Trash2 } from 'lucide-react'

type DownloadEntry = {
  id: string;
  info: any;
}

function toFileUrl(input?: string | null): string {
  if (!input) return ''
  if (input.startsWith('http://') || input.startsWith('https://') || input.startsWith('data:')) return input
  if (input.startsWith('file://')) {
    const raw = input.replace(/^file:\/\//, '').replace(/\//g, '\\')
    return convertFileSrc(raw)
  }
  if (/^[a-zA-Z]:\\/.test(input) || input.startsWith('\\\\')) {
    return convertFileSrc(input)
  }
  return input
}

export const DownloadsPage: React.FC = () => {
  const [downloads, setDownloads] = useState<DownloadEntry[]>([])
  const [activeDownloads, setActiveDownloads] = useState<DownloadEntry[]>([])
  const [loading, setLoading] = useState(false)

  const load = async () => {
    setLoading(true)
    try {
      const res = await apiClient.listDownloadedItems()
      const items = res?.items ?? []
      setDownloads(items.map((i: any) => ({ id: i.id, info: i })))
    } catch (e) {
      console.error('Failed to load downloads', e)
    } finally {
      setLoading(false)
    }
  }

  const loadActiveDownloads = async () => {
    try {
      const res = await apiClient.listDownloads()
      const items = Array.isArray(res?.downloads) ? res.downloads : []
      const active = items.filter((item: any) => {
        const status = String(item?.info?.status || '').toLowerCase()
        return status !== 'completed' && status !== 'failed' && status !== 'error'
      })
      setActiveDownloads(active)
    } catch (e) {
      console.error('Failed to load active downloads', e)
    }
  }

  useEffect(() => {
    let unlisten: UnlistenFn | undefined

    load()
    loadActiveDownloads()

    const pollTimer = setInterval(loadActiveDownloads, 1200)

    ;(async () => {
      try {
        unlisten = await listen('downloads-updated', (e) => {
          const p: any = (e as any).payload
          if (!p) return
          load()
          loadActiveDownloads()
        })
      } catch (err) {
        console.debug('Failed to subscribe to downloads-updated', err)
      }
    })()

    return () => {
      clearInterval(pollTimer)
      if (unlisten) unlisten()
    }
  }, [])

  const onRemove = async (id: string) => {
    if (!confirm('Remover download e arquivos associados?')) return
    try {
      await apiClient.removeDownloadedManga(id)
      await load()
    } catch (e) {
      console.error(e)
      alert('Falha ao remover download')
    }
  }

  const getProgressText = (info: any): string => {
    const last = String(info?.last_stdout || info?.last_stderr || '').trim()
    if (last.length > 0) return `Baixando... ${last}`
    return 'Baixando... preparando capítulos'
  }

  const getProgressNumber = (info: any): number => {
    const raw = Number(info?.progress ?? 0)
    if (!Number.isFinite(raw)) return 0
    return Math.max(0, Math.min(raw, 100))
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold text-dark-text">Downloads</h2>
      </div>

      {loading && <div>Carregando...</div>}

      {!loading && downloads.length === 0 && activeDownloads.length === 0 && (
        <div className="text-sm text-dark-text-secondary">Nenhum download ativo ou concluído.</div>
      )}

      {activeDownloads.length > 0 && (
        <div className="space-y-3">
          <div className="text-sm text-brand-primary font-semibold">Em andamento</div>
          {activeDownloads.map(d => {
            const progress = getProgressNumber(d.info)
            return (
              <div key={d.id} className="bg-dark-surface p-3 rounded-lg border border-dark-border space-y-2">
                <div className="flex items-center justify-between gap-3">
                  <div className="font-semibold text-dark-text">{(d.info.title || d.info.name || 'Download').replace(/_/g, ' ')}</div>
                  <div className="text-xs text-dark-text-secondary">{progress.toFixed(0)}%</div>
                </div>
                <div className="h-2 w-full bg-dark-surface-darker rounded-full overflow-hidden">
                  <div className="h-full bg-brand-primary transition-all" style={{ width: `${progress}%` }} />
                </div>
                <div className="text-xs text-dark-text-secondary break-words">{getProgressText(d.info)}</div>
              </div>
            )
          })}
        </div>
      )}

      <div className="space-y-3">
        {downloads.length > 0 && <div className="text-sm text-dark-text-secondary font-semibold">Concluídos</div>}
        {downloads.map(d => (
          <div key={d.id} className="flex items-center justify-between bg-dark-surface p-3 rounded-lg border border-dark-border">
            <div className="flex items-center gap-3">
              {d.info.coverPath || d.info.coverpath ? (
                <img
                  src={toFileUrl(d.info.coverPath || d.info.coverpath || '')}
                  alt={d.info.title || d.info.name}
                  className="w-16 h-20 object-cover rounded"
                />
              ) : (
                <div className="w-16 h-20 rounded bg-dark-surface-darker border border-dark-border" />
              )}
              <div>
                <div className="font-semibold text-dark-text">
                  {(d.info.title || d.info.name || d.info.localPath?.split(/[\\/]/).pop() || 'Manual Download').replace(/_/g, ' ')}
                </div>
                <div className="text-xs text-dark-text-secondary">{d.info.status || d.info.state || ''}</div>
                <div className="text-xs text-dark-text-secondary">{d.info.localPath || d.info.dest || ''}</div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button title="Mais" className="p-2 rounded hover:bg-dark-surface-darker">
                <MoreHorizontal className="w-5 h-5" />
              </button>
              <button title="Remover" onClick={() => onRemove(d.id)} className="p-2 rounded hover:bg-dark-surface-darker text-red-400">
                <Trash2 className="w-5 h-5" />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
