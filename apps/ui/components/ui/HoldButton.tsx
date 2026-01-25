"use client";

import { useState, useRef, useCallback } from "react";

type HoldButtonVariant = "danger" | "default";

interface HoldButtonProps {
  children: React.ReactNode;
  onComplete: () => void;
  duration?: number;
  variant?: HoldButtonVariant;
  disabled?: boolean;
  className?: string;
}

const variantStyles: Record<HoldButtonVariant, string> = {
  danger: "bg-red-600 text-white border-red-600",
  default: "bg-zinc-700 text-white border-zinc-600",
};

export function HoldButton({
  children,
  onComplete,
  duration = 3000,
  variant = "danger",
  disabled = false,
  className = "",
}: HoldButtonProps) {
  const [holding, setHolding] = useState(false);
  const [progress, setProgress] = useState(0);
  const intervalRef = useRef<NodeJS.Timeout | null>(null);
  const startTimeRef = useRef<number>(0);

  const handleStart = useCallback(() => {
    if (disabled) return;
    setHolding(true);
    startTimeRef.current = Date.now();

    intervalRef.current = setInterval(() => {
      const elapsed = Date.now() - startTimeRef.current;
      const newProgress = Math.min((elapsed / duration) * 100, 100);
      setProgress(newProgress);

      if (newProgress >= 100) {
        handleEnd();
        onComplete();
      }
    }, 50);
  }, [disabled, duration, onComplete]);

  const handleEnd = useCallback(() => {
    setHolding(false);
    setProgress(0);
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  }, []);

  const remainingSeconds = Math.ceil(
    (duration - (progress / 100) * duration) / 1000,
  );

  return (
    <button
      className={`
        relative overflow-hidden rounded-md font-medium border
        px-3 py-1.5 text-sm
        transition-all duration-200
        disabled:opacity-50 disabled:cursor-not-allowed
        focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950
        ${variantStyles[variant]}
        ${className}
      `}
      onMouseDown={handleStart}
      onMouseUp={handleEnd}
      onMouseLeave={handleEnd}
      onTouchStart={handleStart}
      onTouchEnd={handleEnd}
      disabled={disabled}
    >
      <div
        className="absolute inset-0 bg-black/30 transition-all"
        style={{ width: `${progress}%` }}
      />
      <span className="relative z-10 flex items-center gap-2">
        {children}
        {holding && (
          <span className="text-xs opacity-75">({remainingSeconds}s)</span>
        )}
      </span>
    </button>
  );
}
