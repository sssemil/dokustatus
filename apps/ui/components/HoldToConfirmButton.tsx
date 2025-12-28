'use client';

import { useState, useRef, useCallback, useEffect } from 'react';

type HoldToConfirmButtonProps = {
  label: string;
  holdingLabel: string;
  onConfirm: () => void;
  duration?: number;
  variant?: 'danger';
  disabled?: boolean;
  style?: React.CSSProperties;
};

export default function HoldToConfirmButton({
  label,
  holdingLabel,
  onConfirm,
  duration = 3000,
  variant,
  disabled = false,
  style,
}: HoldToConfirmButtonProps) {
  const [isHolding, setIsHolding] = useState(false);
  const [progress, setProgress] = useState(0);
  const startTimeRef = useRef<number | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const confirmedRef = useRef(false);

  const updateProgress = useCallback(() => {
    if (!startTimeRef.current || confirmedRef.current) return;

    const elapsed = Date.now() - startTimeRef.current;
    const newProgress = Math.min(elapsed / duration, 1);
    setProgress(newProgress);

    if (newProgress >= 1) {
      confirmedRef.current = true;
      setIsHolding(false);
      setProgress(0);
      onConfirm();
    } else {
      animationFrameRef.current = requestAnimationFrame(updateProgress);
    }
  }, [duration, onConfirm]);

  const handlePointerDown = useCallback(() => {
    if (disabled) return;
    confirmedRef.current = false;
    startTimeRef.current = Date.now();
    setIsHolding(true);
    animationFrameRef.current = requestAnimationFrame(updateProgress);
  }, [disabled, updateProgress]);

  const handlePointerUp = useCallback(() => {
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current);
    }
    startTimeRef.current = null;
    setIsHolding(false);
    setProgress(0);
  }, []);

  // Clean up on unmount
  useEffect(() => {
    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, []);

  const baseStyle: React.CSSProperties = {
    position: 'relative',
    overflow: 'hidden',
    cursor: disabled ? 'not-allowed' : 'pointer',
    opacity: disabled ? 0.5 : 1,
    backgroundColor: variant === 'danger' ? 'var(--accent-red)' : undefined,
    color: variant === 'danger' ? '#fff' : undefined,
    border: variant === 'danger' ? 'none' : undefined,
    ...style,
  };

  const progressStyle: React.CSSProperties = {
    position: 'absolute',
    top: 0,
    left: 0,
    height: '100%',
    width: `${progress * 100}%`,
    backgroundColor: variant === 'danger'
      ? 'rgba(255, 255, 255, 0.3)'
      : 'rgba(0, 0, 0, 0.1)',
    transition: 'none',
    pointerEvents: 'none',
  };

  const contentStyle: React.CSSProperties = {
    position: 'relative',
    zIndex: 1,
  };

  return (
    <button
      style={baseStyle}
      onPointerDown={handlePointerDown}
      onPointerUp={handlePointerUp}
      onPointerLeave={handlePointerUp}
      onPointerCancel={handlePointerUp}
      disabled={disabled}
    >
      <div style={progressStyle} />
      <span style={contentStyle}>
        {isHolding ? holdingLabel : label}
      </span>
    </button>
  );
}
