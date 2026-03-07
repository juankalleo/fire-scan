import React, { useState } from 'react'
import { api } from '@/utils/apiClient'

export const ManualDownloadPage: React.FC = () => {
  const [url, setUrl] = useState('')
  const [chapters, setChapters] = useState('all')
  const [format, setFormat] = useState('cbz')
  const [status, setStatus] = useState<string | null>(null)

  const handleRun = async () => {
    setStatus('Iniciando...')
    try {
      const res = await api.startDownload(url, chapters, format)
      setStatus(`Iniciado: ${res.download_id} (status: ${res.status})`)
    } catch (e: any) {
      setStatus(`Erro: ${e?.message || e}`)
    }
  }

  return (
    <div className="p-6 space-y-6">
      <h2 className="text-2xl font-bold text-dark-text">Download Manual</h2>

      <div className="space-y-3 max-w-lg">
        <label className="block text-sm text-dark-text-secondary">Link (cole o link do Kotatsu)</label>
        <input value={url} onChange={(e) => setUrl(e.target.value)} className="w-full p-2 bg-dark-surface border border-dark-border rounded" placeholder="https://..." />

        <label className="block text-sm text-dark-text-secondary">Capítulos (ex: 1-4,8 ou all)</label>
        <input value={chapters} onChange={(e) => setChapters(e.target.value)} className="w-full p-2 bg-dark-surface border border-dark-border rounded" />

        <label className="block text-sm text-dark-text-secondary">Formato (cbz | zip | dir)</label>
        <input value={format} onChange={(e) => setFormat(e.target.value)} className="w-32 p-2 bg-dark-surface border border-dark-border rounded" />

        <div className="flex gap-2">
          <button onClick={handleRun} className="px-4 py-2 bg-brand-primary text-white rounded">Run</button>
          <button onClick={() => { setUrl(''); setChapters('all'); setFormat('cbz'); setStatus(null); }} className="px-4 py-2 bg-dark-surface border rounded">Reset</button>
        </div>

        {status && <div className="mt-2 p-3 bg-dark-surface border border-dark-border rounded text-sm">{status}</div>}
      </div>
    </div>
  )
}
