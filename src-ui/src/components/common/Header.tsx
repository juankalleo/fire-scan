import React from 'react'
import { Sun, Moon, X, Minus, Square } from 'lucide-react'
import { appWindow } from '@tauri-apps/api/window'

interface HeaderProps {
  title: string
  theme: 'light' | 'dark'
  onThemeToggle: () => void
}

export const Header: React.FC<HeaderProps> = ({ title, theme, onThemeToggle }) => {
  const handleMinimize = async () => {
    await appWindow.minimize()
  }

  const handleMaximize = async () => {
    await appWindow.toggleMaximize()
  }

  const handleClose = async () => {
    await appWindow.close()
  }

  return (
    <header className="relative flex items-center justify-between px-6 py-4 bg-dark-surface/85 border-b border-dark-border/80 backdrop-blur-md">
      <div className="pointer-events-none absolute inset-0">
        <div className="absolute left-10 -top-16 h-32 w-48 rounded-full bg-brand-primary/10 blur-3xl" />
      </div>

      <h1 className="relative text-lg font-semibold tracking-tight text-dark-text">{title}</h1>

      <div className="relative flex items-center gap-2">
        <button
          onClick={onThemeToggle}
          className="btn-ghost p-2 rounded-xl border border-dark-border/60"
          title={theme === 'dark' ? 'Modo Claro' : 'Modo Escuro'}
        >
          {theme === 'dark' ? (
            <Sun className="w-5 h-5" />
          ) : (
            <Moon className="w-5 h-5" />
          )}
        </button>

        {/* Window Controls */}
        <div className="flex items-center gap-1 rounded-xl border border-dark-border/60 bg-dark-surface-alt/40 px-1" data-tauri-drag-region>
          <button
            onClick={handleMinimize}
            className="btn-ghost p-2 hover:text-brand-primary rounded-lg"
            title="Minimizar"
          >
            <Minus className="w-4 h-4" />
          </button>
          <button
            onClick={handleMaximize}
            className="btn-ghost p-2 hover:text-brand-primary rounded-lg"
            title="Maximizar"
          >
            <Square className="w-4 h-4" />
          </button>
          <button
            onClick={handleClose}
            className="btn-ghost p-2 hover:text-red-400 rounded-lg"
            title="Fechar"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      </div>
    </header>
  )
}
