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
    { id: 'light' as const, icon: Sun },
    { id: 'dark' as const, icon: Moon },
    { id: 'system' as const, icon: Monitor },
  ];

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

          {/* Theme toggle */}
          <div className="border-t border-zinc-700 px-3 py-2.5 flex items-center justify-center">
            <div className="relative flex bg-zinc-900 rounded-lg p-1">
              {/* Sliding pill indicator */}
              <div
                className="absolute top-1 h-[calc(100%-8px)] w-[calc(33.333%-2px)] bg-zinc-700 rounded-md shadow-sm transition-all duration-200 ease-out"
                style={{
                  left: `calc(${themes.findIndex(t => t.id === theme) * 33.333}% + 4px)`,
                }}
              />
              {themes.map((t) => {
                const isActive = theme === t.id;
                return (
                  <button
                    key={t.id}
                    onClick={() => setTheme(t.id)}
                    className={`
                      relative z-10 p-2 rounded-md transition-colors duration-200
                      ${isActive ? 'text-white' : 'text-zinc-500 hover:text-zinc-300'}
                    `}
                    title={t.id.charAt(0).toUpperCase() + t.id.slice(1)}
                  >
                    <t.icon size={16} />
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
