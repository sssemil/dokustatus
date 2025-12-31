import { RefreshCw } from 'lucide-react';

interface ToggleProps {
  enabled: boolean;
  onChange: (enabled: boolean) => void;
  disabled?: boolean;
  saving?: boolean;
  label?: string;
}

export function Toggle({ enabled, onChange, disabled = false, saving = false, label }: ToggleProps) {
  const handleClick = () => {
    if (!disabled && !saving) {
      onChange(!enabled);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === ' ' || e.key === 'Enter') {
      e.preventDefault();
      handleClick();
    }
  };

  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        role="switch"
        aria-checked={enabled}
        onClick={handleClick}
        onKeyDown={handleKeyDown}
        disabled={disabled || saving}
        className={`
          relative w-10 h-6 rounded-full transition-all duration-200
          focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950
          ${enabled ? 'bg-blue-600 border border-blue-500' : 'bg-zinc-600 border border-zinc-500'}
          ${disabled || saving ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
        `}
      >
        <div
          className={`
            absolute top-1 w-4 h-4 bg-white rounded-full
            transition-all duration-200 flex items-center justify-center
            ${enabled ? 'left-5' : 'left-1'}
          `}
        >
          {saving && <RefreshCw size={10} className="animate-spin text-zinc-600" />}
        </div>
      </button>
      {label && (
        <span className="text-sm text-zinc-300">{label}</span>
      )}
    </div>
  );
}
