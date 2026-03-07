import React, { useState, useEffect } from 'react'
import { api } from '../utils/apiClient'
import { open } from '@tauri-apps/api/dialog'
import { useToast, ToastContainer } from '../components/common/Toast'

export const SettingsPage: React.FC = () => {
  const [theme, setTheme] = useState('dark')
  const [concurrency, setConcurrency] = useState(4)
  const [libraryPath, setLibraryPath] = useState('')

  const { toasts, addToast, removeToast } = useToast()

  useEffect(() => {
    let mounted = true
    api.getLibraryPath().then((res) => {
      if (mounted && res && res.library_path) setLibraryPath(res.library_path)
    }).catch(() => {})
    return () => { mounted = false }
  }, [])

  return (
    <div className="p-6 max-w-2xl space-y-6">
      <h2 className="text-2xl font-bold text-dark-text">Configurações</h2>

      {/* General Settings */}
      <div className="card p-6 space-y-4">
        <h3 className="text-lg font-semibold text-dark-text">Geral</h3>

            <div className="space-y-3">
          <div>
            <label className="block text-sm text-dark-text-secondary mb-2">Tema</label>
            <select
              value={theme}
              onChange={(e) => setTheme(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-dark-surface border border-dark-border text-dark-text"
            >
              <option value="dark">Escuro</option>
              <option value="light">Claro</option>
              <option value="auto">Automático</option>
            </select>
          </div>

          <div>
            <label className="block text-sm text-dark-text-secondary mb-2">Localização da Biblioteca</label>
            <div className="flex gap-2">
              <input
                type="text"
                value={libraryPath}
                onChange={(e) => setLibraryPath(e.target.value)}
                className="flex-1 px-3 py-2 rounded-lg bg-dark-surface-alt border border-dark-border text-dark-text"
              />
              <button
                className="btn-secondary"
                onClick={async () => {
                  try {
                    const selected = await open({ directory: true })
                    if (!selected) return
                    // selected may be string or array
                    const path = Array.isArray(selected) ? selected[0] : selected
                    // only update local input here; don't persist immediately
                    setLibraryPath(path as string)
                  } catch (e) {
                    console.error('Directory picker failed:', e)
                    addToast('Falha ao selecionar pasta', 'error')
                  }
                }}
              >
                Procurar
              </button>

              <button
                className="btn-primary"
                onClick={async () => {
                  try {
                    if (!libraryPath || libraryPath.trim() === '') {
                      addToast('Nenhum caminho selecionado', 'error')
                      return
                    }
                    await api.setLibraryPath(libraryPath)
                    addToast('Caminho da biblioteca alterado', 'success')
                  } catch (e) {
                    console.error('Failed to persist library path', e)
                    const msg = e instanceof Error ? e.message : String(e)
                    addToast(`Falha ao salvar caminho: ${msg}`, 'error')
                  }
                }}
              >
                Confirmar
              </button>
            </div>
          </div>
          <ToastContainer toasts={toasts} onClose={removeToast} />
        </div>
      </div>

      {/* Download Settings */}
      <div className="card p-6 space-y-4">
        <h3 className="text-lg font-semibold text-dark-text">Downloads</h3>

        <div className="space-y-3">
          <div>
            <label className="block text-sm text-dark-text-secondary mb-2">
              Downloads Concorrentes: {concurrency}
            </label>
            <input
              type="range"
              min="1"
              max="10"
              value={concurrency}
              onChange={(e) => setConcurrency(Number(e.target.value))}
              className="w-full"
            />
          </div>

          <div>
            <label className="block text-sm text-dark-text-secondary mb-2">Throttle (ms)</label>
            <input
              type="number"
              defaultValue={500}
              className="w-full px-3 py-2 rounded-lg bg-dark-surface border border-dark-border text-dark-text"
            />
          </div>

          <div>
            <label className="block text-sm text-dark-text-secondary mb-2">Formato</label>
            <select className="w-full px-3 py-2 rounded-lg bg-dark-surface border border-dark-border text-dark-text">
              <option>CBZ (Comic Book Archive)</option>
              <option>ZIP</option>
              <option>Diretório (DIR)</option>
            </select>
          </div>
        </div>
      </div>

      {/* Action Buttons */}
      <div className="flex gap-3 justify-end">
        <button className="btn-secondary">Cancelar</button>
        <button className="btn-primary">Salvar</button>
      </div>
    </div>
  )
}
