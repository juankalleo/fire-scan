import React from 'react'
import { SpinnerLoader } from '@/components/common/Loading'
import { useSources } from '@/hooks/useSources'
import { CheckCircle, XCircle, RefreshCw } from 'lucide-react'
import mangalivreLogo from '@/assets/icons/mangalivrelogo.png'
import niaddLogo from '@/assets/icons/niaddlogo.jfif'

const getSourceLogo = (sourceId: string): string | null => {
  const key = sourceId.toLowerCase()
  if (key === 'mangalivre') return mangalivreLogo
  if (key === 'niadd') return niaddLogo
  return null
}

const getInitials = (name: string): string =>
  name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? '')
    .join('')

export const SourcesPage: React.FC = () => {
  const { sources, isLoading, error, total, enabled, refresh, toggleSource } = useSources()

  const handleToggle = (sourceId: string, currentEnabled: boolean) => {
    toggleSource(sourceId, !currentEnabled)
  }

  const enableAll = () => {
    sources.forEach(s => {
      if (!s.enabled) toggleSource(s.id, true)
    })
  }

  if (isLoading) {
    return (
      <div className="p-6">
        <h2 className="text-2xl font-bold text-dark-text mb-6">Fontes</h2>
        <div className="card p-12 flex justify-center">
          <SpinnerLoader />
        </div>
      </div>
    )
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex-1">
          <h2 className="text-2xl font-bold text-dark-text">
            Fontes de Mangá
            {total > 0 && (
              <span className="text-dark-text-secondary text-lg ml-2">
                ({enabled}/{total} ativas)
              </span>
            )}
          </h2>
        </div>
        <button 
          onClick={refresh}
          disabled={isLoading}
          className="p-2 rounded-lg bg-dark-surface border border-dark-border text-dark-text hover:border-brand-primary disabled:opacity-50 transition-colors"
          title="Atualizar"
        >
          <RefreshCw className={`w-5 h-5 ${isLoading ? 'animate-spin' : ''}`} />
        </button>
        <button 
          onClick={enableAll}
          className="ml-3 px-4 py-2 bg-brand-primary hover:bg-brand-primary/80 text-white rounded-lg transition text-sm font-medium"
        >
          Ativar Tudo
        </button>
      </div>

      {/* Error State */}
      {error && (
        <div className="bg-red-900/20 border border-red-500/30 rounded-lg p-4 text-red-300">
          <p>Erro ao carregar fontes: {error}</p>
        </div>
      )}

      {sources.length === 0 && !error ? (
        <div className="card p-8 text-center">
          <p className="text-dark-text-secondary">Nenhuma fonte disponível</p>
        </div>
      ) : (
        <div className="grid gap-3">
          {sources.map(source => (
            <div 
              key={source.id} 
              className="card p-4 flex items-center justify-between hover:bg-dark-surface-darker transition"
            >
              <div className="flex items-center gap-3 flex-1 min-w-0">
                {getSourceLogo(source.id) ? (
                  <div className="h-11 w-11 rounded-xl overflow-hidden border border-dark-border/70 bg-dark-surface-alt flex-shrink-0">
                    <img
                      src={getSourceLogo(source.id) as string}
                      alt={source.name}
                      className="h-full w-full object-cover"
                      loading="lazy"
                    />
                  </div>
                ) : (
                  <div className="h-11 w-11 rounded-xl border border-dark-border/70 bg-dark-surface-alt flex items-center justify-center text-xs font-bold text-dark-text-secondary flex-shrink-0">
                    {getInitials(source.name || source.id)}
                  </div>
                )}

                <div className="flex-1 min-w-0">
                <h3 className="font-semibold text-dark-text truncate">{source.name}</h3>
                <div className="flex gap-4 text-sm text-dark-text-secondary mt-2">
                  <span>{source.language}</span>
                  <span>{source.region}</span>
                  {source.description && (
                    <span>{source.description}</span>
                  )}
                </div>
                </div>
              </div>
              
              <button
                onClick={() => handleToggle(source.id, source.enabled)}
                className="ml-4 transition-colors flex-shrink-0"
                title={source.enabled ? 'Desativar' : 'Ativar'}
              >
                {source.enabled ? (
                  <CheckCircle className="w-6 h-6 text-green-500 hover:text-green-400" />
                ) : (
                  <XCircle className="w-6 h-6 text-gray-500 hover:text-gray-400" />
                )}
              </button>
            </div>
          ))}
        </div>
      )}

      <div className="card p-4">
        <h3 className="font-semibold text-dark-text mb-2">Informação</h3>
        <p className="text-sm text-dark-text-secondary">
          As fontes selecionadas serão usadas ao buscar por novos mangás. 
          Você pode ativar/desativar fontes clicando no ícone ao lado.
        </p>
        {total > 0 && (
          <p className="text-sm text-dark-text-secondary mt-2">
            Total de {total} fontes disponíveis. {enabled} ativas.
          </p>
        )}
      </div>
    </div>
  )
}
