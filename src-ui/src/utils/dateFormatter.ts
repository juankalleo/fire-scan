import { formatDistance, format, parseISO } from 'date-fns'
import { ptBR } from 'date-fns/locale'

/**
 * Format date in PT-BR locale
 */
export const formatDatePtBr = (date: Date | string): string => {
  const d = typeof date === 'string' ? parseISO(date) : date
  return format(d, "dd 'de' MMMM 'de' yyyy", { locale: ptBR })
}

/**
 * Format relative time (e.g. "2 horas atrás")
 */
export const formatRelativeTime = (date: Date | string): string => {
  const d = typeof date === 'string' ? parseISO(date) : date
  return formatDistance(d, new Date(), { addSuffix: true, locale: ptBR })
}

/**
 * Format file size in human-readable format
 */
export const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}

/**
 * Format percentage for display
 */
export const formatPercent = (value: number): string => {
  return Math.min(100, Math.max(0, Math.round(value))).toString() + '%'
}

/**
 * Format chapter number (e.g. "Cap. 45" or "Vol 1, Cap 3")
 */
export const formatChapterNumber = (number: number, volume?: number): string => {
  if (!volume) {
    return `Cap. ${number.toFixed(number % 1 !== 0 ? 1 : 0)}`
  }
  return `Vol ${volume}, Cap ${number.toFixed(number % 1 !== 0 ? 1 : 0)}`
}

/**
 * Format title for folder names (sanitize)
 */
export const sanitizeFileName = (title: string): string => {
  return title
    .replace(/[<>:"/\\|?*]/g, '')
    .replace(/\s+/g, '_')
    .trim()
}
