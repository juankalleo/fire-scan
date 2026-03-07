import React from 'react'
import { Home, Search, Heart, Book, Settings, Download } from 'lucide-react'
import appIcon from '@/assets/icons/icon.png'

interface SidebarProps {
  currentPage: string
  onPageChange: (page: string) => void
  downloadCount?: number
}

const sidebarItems = [
  { id: 'library', label: 'Biblioteca', icon: Home },
  { id: 'search', label: 'Buscar', icon: Search },
  { id: 'favorites', label: 'Favoritos', icon: Heart },
  { id: 'sources', label: 'Fontes', icon: Book },
  { id: 'downloads', label: 'Downloads', icon: Download },
  { id: 'manual-download', label: 'Download Manual', icon: Download },
  { id: 'settings', label: 'Configurações', icon: Settings },
]

export const Sidebar: React.FC<SidebarProps> = ({ currentPage, onPageChange, downloadCount = 0 }) => {
  return (
    <aside className="relative w-48 bg-dark-surface/85 backdrop-blur-md border-r border-dark-border/40 flex flex-col h-screen overflow-hidden">

      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(120%_60%_at_0%_0%,rgba(95,157,255,0.12),transparent_48%),radial-gradient(120%_60%_at_100%_100%,rgba(52,211,153,0.08),transparent_45%)]" />

      {/* Logo */}
      <div className="relative px-3.5 py-[18px]">
        <div className="flex items-center gap-2">
          <div className="flex h-9 w-9 items-center justify-center rounded-xl border border-brand-primary/25 bg-brand-primary/10 shadow-[0_6px_20px_rgba(58,123,255,0.22)] overflow-hidden">
            <img src={appIcon} alt="FireScan" className="h-full w-full object-cover" />
          </div>
          <h1 className="text-base font-bold tracking-tight text-dark-text">FireScan</h1>
        </div>

        <div className="mt-3 h-px bg-gradient-to-r from-transparent via-dark-border/35 to-transparent" />
      </div>

      {/* Navigation Items */}
      <nav className="relative flex-1 px-2 py-3.5 space-y-1 overflow-y-auto">
        {sidebarItems.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => onPageChange(id)}
            className={`sidebar-item w-full justify-start rounded-xl ${
              currentPage === id ? 'active' : ''
            }`}
          >
            <Icon className="w-[16px] h-[16px] flex-shrink-0" />
            <span className="text-[13px] leading-none">{label}</span>
            {id === 'downloads' && downloadCount > 0 && (
              <span className="ml-auto bg-brand-primary text-dark-bg text-[10px] font-bold rounded-full w-[18px] h-[18px] flex items-center justify-center">
                {downloadCount}
              </span>
            )}
          </button>
        ))}
      </nav>

      {/* Footer */}
      <div className="relative px-3.5 py-3 text-[10px] text-dark-text-secondary/90">
        <div className="mb-2 h-px bg-gradient-to-r from-transparent via-dark-border/30 to-transparent" />
        <p>FireScan v0.2.0</p>
        <p className="mt-0.5">© 2026 All rights reserved</p>
      </div>
    </aside>
  )
}
