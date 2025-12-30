/**
 * Design tokens for the UI
 * These complement Tailwind's built-in scales
 */

export const zIndex = {
  dropdown: 50,
  sticky: 60,
  modal: 100,
  toast: 150,
} as const;

// For programmatic access when needed
export const colors = {
  background: {
    primary: 'rgb(9 9 11)', // zinc-950
    secondary: 'rgb(24 24 27)', // zinc-900
    tertiary: 'rgb(39 39 42)', // zinc-800
  },
  accent: {
    blue: 'rgb(59 130 246)', // blue-500
    green: 'rgb(34 197 94)', // green-500
    amber: 'rgb(245 158 11)', // amber-500
    red: 'rgb(239 68 68)', // red-500
  },
} as const;

// Animation durations
export const durations = {
  fast: 150,
  normal: 200,
  slow: 300,
} as const;
