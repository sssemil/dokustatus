interface ProgressBarProps {
  value: number;
  max: number;
  className?: string;
  showLabel?: boolean;
}

export function ProgressBar({ value, max, className = '', showLabel = false }: ProgressBarProps) {
  const percentage = Math.min((value / max) * 100, 100);
  const isHigh = percentage > 80;

  return (
    <div className={className}>
      <div className="h-2 bg-zinc-800 rounded-full overflow-hidden border border-zinc-700">
        <div
          className={`h-full transition-all duration-500 rounded-full ${
            isHigh ? 'bg-amber-500' : 'bg-blue-500'
          }`}
          style={{ width: `${percentage}%` }}
          role="progressbar"
          aria-valuenow={value}
          aria-valuemin={0}
          aria-valuemax={max}
        />
      </div>
      {showLabel && (
        <div className="flex justify-between mt-1 text-xs text-zinc-500">
          <span>{value} used</span>
          <span>{max - value} remaining</span>
        </div>
      )}
    </div>
  );
}
