import { ButtonHTMLAttributes, forwardRef, ReactNode } from 'react';
import { RefreshCw } from 'lucide-react';

type ButtonVariant = 'default' | 'primary' | 'danger' | 'ghost' | 'outline';
type ButtonSize = 'sm' | 'md' | 'lg';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  children: ReactNode;
}

const variantStyles: Record<ButtonVariant, string> = {
  default: 'bg-zinc-700 hover:bg-zinc-600 text-white border-zinc-600',
  primary: 'bg-blue-600 hover:bg-blue-500 text-white border-blue-600',
  danger: 'bg-red-600/20 hover:bg-red-600/30 text-red-400 border-red-600/50',
  ghost: 'bg-transparent hover:bg-zinc-800 text-zinc-400 hover:text-white border-transparent',
  outline: 'bg-transparent hover:bg-zinc-800 text-zinc-300 border-zinc-600',
};

const sizeStyles: Record<ButtonSize, string> = {
  sm: 'px-2 py-1 text-xs gap-1',
  md: 'px-3 py-1.5 text-sm gap-2',
  lg: 'px-4 py-2 text-base gap-2',
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ variant = 'default', size = 'md', loading = false, disabled, children, className = '', ...props }, ref) => {
    return (
      <button
        ref={ref}
        disabled={disabled || loading}
        className={`
          inline-flex items-center justify-center rounded-md font-medium border
          transition-all duration-200
          disabled:opacity-50 disabled:cursor-not-allowed
          focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950
          ${variantStyles[variant]}
          ${sizeStyles[size]}
          ${className}
        `}
        {...props}
      >
        {loading && <RefreshCw size={14} className="animate-spin" />}
        {children}
      </button>
    );
  }
);

Button.displayName = 'Button';
