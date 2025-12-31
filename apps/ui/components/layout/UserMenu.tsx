'use client';

import { useState, useEffect, useRef } from 'react';
import { User, LogOut, Sun, Moon, Monitor, ChevronDown } from 'lucide-react';
import { useTheme } from '@/app/components/ThemeContext';
import { zIndex } from '@/lib/design-tokens';

interface UserMenuProps {
  email: string;
  collapsed?: boolean;
  onLogout: () => void;
  onProfileClick: () => void;
}

export function UserMenu({ email, collapsed = false, onLogout, onProfileClick }: UserMenuProps) {
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const { theme, setTheme } = useTheme();

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };

    if (open) {
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [open]);

  // Get initials from email
  const initials = email.charAt(0).toUpperCase();

  const themes = [
    { id: 'light' as const, icon: Sun, label: 'Light' },
    { id: 'dark' as const, icon: Moon, label: 'Dark' },
    { id: 'system' as const, icon: Monitor, label: 'System' },
  ];

  const calculatePillLeft = (activeIndex: number): string => {
    // flex-[2] for active (50%), flex-1 for inactive (25% each)
    const positions = ['0%', '25%', '50%'];
    return positions[activeIndex];
  };

  return (
    <div ref={menuRef} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className={`
          w-full flex items-center gap-2 px-3 py-2 text-sm text-zinc-400
          hover:text-white hover:bg-zinc-800 rounded-md transition-all duration-200
          ${collapsed ? 'justify-center' : 'justify-between'}
        `}
      >
        <div className="flex items-center gap-2">
          <div className="w-7 h-7 bg-gradient-to-br from-blue-500 to-purple-600 rounded-full flex items-center justify-center text-xs font-medium text-white flex-shrink-0">
            {initials}
          </div>
          {!collapsed && (
            <span className="truncate max-w-[120px]">{email}</span>
          )}
        </div>
        {!collapsed && (
          <ChevronDown
            size={14}
            className={`transition-transform duration-200 ${open ? 'rotate-180' : ''}`}
          />
        )}
      </button>

      {open && (
        <div
          className={`
            absolute bottom-full mb-1 bg-zinc-800 border border-zinc-700
            rounded-lg shadow-xl overflow-hidden animate-scale-in
            ${collapsed ? 'left-0 min-w-[180px]' : 'left-0 right-0'}
          `}
          style={{ zIndex: zIndex.dropdown }}
        >
          {/* Profile */}
          <button
            onClick={() => {
              onProfileClick();
              setOpen(false);
            }}
            className="w-full flex items-center gap-2 px-3 py-2.5 text-sm text-zinc-300 hover:bg-zinc-700 transition-colors"
          >
            <User size={16} />
            My profile
          </button>

          {/* Theme toggle - full width */}
          <div className="border-t border-zinc-700 px-2 py-2">
            <div className="relative flex bg-zinc-900 rounded-lg p-1 w-full overflow-hidden">
              {/* Sliding pill - pointer-events-none to not block clicks */}
              <div
                className="absolute top-1 bottom-1 bg-zinc-700 rounded-md pointer-events-none transition-[left] duration-300 ease-out"
                style={{
                  width: '50%',
                  left: calculatePillLeft(themes.findIndex(t => t.id === theme)),
                }}
              />
              {themes.map((t) => {
                const isActive = theme === t.id;
                return (
                  <button
                    key={t.id}
                    onClick={() => setTheme(t.id)}
                    aria-label={`${t.label} theme`}
                    className={`
                      relative z-10 flex items-center justify-center py-1.5 rounded-md
                      transition-[flex-grow,color] duration-300 ease-out
                      ${isActive ? 'flex-[2] text-white' : 'flex-1 text-zinc-500 hover:text-zinc-300'}
                    `}
                  >
                    <t.icon size={14} />
                    {/* Label with margin instead of gap - avoids gap on 0-width element */}
                    <span className={`
                      text-xs font-medium overflow-hidden whitespace-nowrap
                      transition-[opacity,max-width,margin] duration-300 ease-out
                      ${isActive ? 'opacity-100 max-w-[60px] ml-1.5' : 'opacity-0 max-w-0 ml-0'}
                    `}>
                      {t.label}
                    </span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Logout */}
          <button
            onClick={() => {
              onLogout();
              setOpen(false);
            }}
            className="w-full flex items-center gap-2 px-3 py-2.5 text-sm text-red-400 hover:bg-zinc-700 border-t border-zinc-700 transition-colors"
          >
            <LogOut size={16} />
            Log out
          </button>
        </div>
      )}
    </div>
  );
}
