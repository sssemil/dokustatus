import { ReactNode, HTMLAttributes } from 'react';

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  hover?: boolean;
}

export function Card({ children, hover = false, className = '', onClick, ...props }: CardProps) {
  return (
    <div
      className={`
        bg-zinc-800/50 border border-zinc-700 rounded-lg
        transition-all duration-200
        ${hover ? 'hover:border-zinc-500 hover:bg-zinc-800/70 cursor-pointer' : ''}
        ${className}
      `}
      onClick={onClick}
      {...props}
    >
      {children}
    </div>
  );
}
