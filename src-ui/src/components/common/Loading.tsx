import React from 'react'

export const SkeletonLoader: React.FC = () => {
  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6">
      {Array.from({ length: 8 }).map((_, i) => (
        <div key={i} className="card p-4 space-y-4">
          <div className="bg-dark-surface-alt rounded-lg w-full aspect-video animate-pulse" />
          <div className="space-y-2">
            <div className="h-4 bg-dark-surface-alt rounded animate-pulse" />
            <div className="h-3 bg-dark-surface-alt rounded w-2/3 animate-pulse" />
          </div>
        </div>
      ))}
    </div>
  )
}

export const SpinnerLoader: React.FC = () => {
  return (
    <div className="flex items-center justify-center h-64">
      <div className="relative w-12 h-12">
        <div className="absolute inset-0 rounded-full border-4 border-dark-surface-alt" />
        <div className="absolute inset-0 rounded-full border-4 border-transparent border-t-brand-primary animate-spin" />
      </div>
    </div>
  )
}

export const EmptyState: React.FC<{ title: string; description: string }> = ({
  title,
  description,
}) => {
  return (
    <div className="flex flex-col items-center justify-center h-64 text-center">
      <p className="text-xl font-semibold text-dark-text">{title}</p>
      <p className="text-dark-text-secondary mt-2">{description}</p>
    </div>
  )
}
