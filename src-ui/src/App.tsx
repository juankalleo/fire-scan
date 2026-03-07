import React, { useState } from 'react'
import { Sidebar } from '@/components/common/Sidebar'
import { ToastContainer, useToast } from '@/components/common/Toast'

import { LibraryPage } from '@/pages/Library'
import { SearchPage } from '@/pages/Search'
import { FavoritesPage } from '@/pages/Favorites'
import { DownloadsPage } from '@/pages/Downloads'
import { SourcesPage } from '@/pages/Sources'
import { SettingsPage } from '@/pages/Settings'
import { ManualDownloadPage } from '@/pages/ManualDownload'

import '@/styles/globals.css'

type PageType = 'library' | 'search' | 'favorites' | 'sources' | 'downloads' | 'settings' | 'manual-download'

const pageConfig: Record<
  PageType,
  {
    title: string
    component: React.ComponentType<any>
  }
> = {
  library: { title: 'Biblioteca', component: LibraryPage },
  search: { title: 'Buscar', component: SearchPage },
  favorites: { title: 'Favoritos', component: FavoritesPage },
  sources: { title: 'Fontes', component: SourcesPage },
  downloads: { title: 'Downloads', component: DownloadsPage },
  'manual-download': { title: 'Download Manual', component: ManualDownloadPage },
  settings: { title: 'Configurações', component: SettingsPage },
}

export const App: React.FC = () => {
  const [currentPage, setCurrentPage] = useState<PageType>('library')
  const { toasts, removeToast } = useToast()

  const currentConfig = pageConfig[currentPage]
  const CurrentComponent = currentConfig.component

  return (
    <div className="flex h-screen bg-dark-bg">
      <Sidebar currentPage={currentPage} onPageChange={(page) => setCurrentPage(page as PageType)} />

      <div className="flex-1 overflow-hidden">
        <main className="h-full overflow-y-auto">
          <CurrentComponent />
        </main>
      </div>

      <ToastContainer toasts={toasts} onClose={removeToast} />
    </div>
  )
}

export default App
