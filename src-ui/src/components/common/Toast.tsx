import React, { useEffect, useState } from 'react'
import { X, CheckCircle, AlertCircle, Info } from 'lucide-react'

export type ToastType = 'success' | 'error' | 'info' | 'warning'

export interface Toast {
  id: string
  message: string
  type: ToastType
  duration?: number
}

interface ToastProps {
  toast: Toast
  onClose: (id: string) => void
}

const toastConfig: Record<ToastType, { icon: React.ReactNode; bgColor: string; textColor: string }> = {
  success: { icon: <CheckCircle />, bgColor: 'bg-green-900/20', textColor: 'text-green-300' },
  error: { icon: <AlertCircle />, bgColor: 'bg-red-900/20', textColor: 'text-red-300' },
  info: { icon: <Info />, bgColor: 'bg-blue-900/20', textColor: 'text-blue-300' },
  warning: { icon: <AlertCircle />, bgColor: 'bg-yellow-900/20', textColor: 'text-yellow-300' },
}

const ToastItem: React.FC<ToastProps> = ({ toast, onClose }) => {
  useEffect(() => {
    const duration = toast.duration || 4000
    const timer = setTimeout(() => onClose(toast.id), duration)
    return () => clearTimeout(timer)
  }, [toast, onClose])

  const config = toastConfig[toast.type]

  return (
    <div className={`flex items-center gap-3 px-4 py-3 rounded-lg ${config.bgColor} border border-dark-border`}>
      <div className={config.textColor}>{config.icon}</div>
      <p className="text-sm text-dark-text flex-1">{toast.message}</p>
      <button
        onClick={() => onClose(toast.id)}
        className="text-dark-text-secondary hover:text-dark-text transition-colors"
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  )
}

interface ToastContainerProps {
  toasts: Toast[]
  onClose: (id: string) => void
}

export const ToastContainer: React.FC<ToastContainerProps> = ({ toasts, onClose }) => {
  return (
    <div className="fixed bottom-6 right-6 space-y-3 z-50 pointer-events-none">
      {toasts.map((toast) => (
        <div key={toast.id} className="pointer-events-auto">
          <ToastItem toast={toast} onClose={onClose} />
        </div>
      ))}
    </div>
  )
}

/**
 * Hook to manage toasts
 */
export const useToast = () => {
  const [toasts, setToasts] = useState<Toast[]>([])

  const addToast = (message: string, type: ToastType = 'info', duration?: number) => {
    const id = Date.now().toString()
    setToasts((prev) => [...prev, { id, message, type, duration }])
  }

  const removeToast = (id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id))
  }

  return { toasts, addToast, removeToast }
}
