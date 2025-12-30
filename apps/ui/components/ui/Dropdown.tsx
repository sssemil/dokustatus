'use client';

import { ReactNode, useEffect, useRef, useState } from 'react';
import { zIndex } from '@/lib/design-tokens';

interface DropdownItem {
  label: string;
  onClick: () => void;
  icon?: ReactNode;
  variant?: 'default' | 'danger';
  divider?: boolean;
}

interface DropdownProps {
  trigger: ReactNode;
  items: DropdownItem[];
  align?: 'left' | 'right';
  className?: string;
}

export function Dropdown({ trigger, items, align = 'right', className = '' }: DropdownProps) {
  const [open, setOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setOpen(false);
      }
    };

    if (open) {
      document.addEventListener('mousedown', handleClickOutside);
      document.addEventListener('keydown', handleEscape);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [open]);

  return (
    <div ref={dropdownRef} className={`relative ${className}`}>
      <div onClick={() => setOpen(!open)}>{trigger}</div>

      {open && (
        <div
          className={`
            absolute top-full mt-1 min-w-[160px]
            bg-zinc-800 border border-zinc-700 rounded-lg shadow-xl
            py-1 animate-scale-in
            ${align === 'right' ? 'right-0' : 'left-0'}
          `}
          style={{ zIndex: zIndex.dropdown }}
        >
          {items.map((item, index) => (
            <div key={index}>
              {item.divider && <div className="border-t border-zinc-700 my-1" />}
              <button
                onClick={() => {
                  item.onClick();
                  setOpen(false);
                }}
                className={`
                  w-full flex items-center gap-2 px-3 py-2 text-sm text-left
                  transition-colors duration-150
                  ${
                    item.variant === 'danger'
                      ? 'text-red-400 hover:bg-red-900/30'
                      : 'text-zinc-300 hover:bg-zinc-700'
                  }
                `}
              >
                {item.icon}
                {item.label}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
