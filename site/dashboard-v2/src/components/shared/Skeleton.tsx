export function Skeleton({ className = '' }: { className?: string }) {
  return <div className={`shimmer ${className}`} />;
}

export function SkeletonCard() {
  return (
    <div className="bg-dag-card border border-dag-border rounded-xl p-5">
      <Skeleton className="h-3 w-20 mb-3" />
      <Skeleton className="h-7 w-32 mb-2" />
      <Skeleton className="h-3 w-24" />
    </div>
  );
}
