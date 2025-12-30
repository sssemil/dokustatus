interface SkeletonProps {
  className?: string;
}

export function Skeleton({ className = '' }: SkeletonProps) {
  return (
    <div
      className={`bg-zinc-700/50 animate-pulse rounded ${className}`}
      aria-hidden="true"
    />
  );
}
