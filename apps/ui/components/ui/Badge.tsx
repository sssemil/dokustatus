import { ReactNode } from 'react';

type BadgeVariant = 'default' | 'success' | 'error' | 'warning' | 'info';

interface BadgeProps {
  children: ReactNode;
  variant?: BadgeVariant;
  className?: string;
}

const variantStyles: Record<BadgeVariant, string> = {
  default: 'bg-zinc-700 text-zinc-300 border border-zinc-600',
  success: 'bg-emerald-900/50 text-emerald-400 border border-emerald-700',
  error: 'bg-red-900/50 text-red-400 border border-red-700',
  warning: 'bg-amber-900/50 text-amber-400 border border-amber-700',
  info: 'bg-blue-900/50 text-blue-400 border border-blue-700',
};

export function Badge({ children, variant = 'default', className = '' }: BadgeProps) {
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${variantStyles[variant]} ${className}`}
    >
      {children}
    </span>
  );
}
