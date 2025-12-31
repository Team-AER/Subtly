import React from 'react';
import { cn } from '@/lib/utils';

export function Progress({ value = 0, max = 100, className, ...props }) {
  const percentage = Math.min(100, Math.max(0, (value / max) * 100));

  return (
    <div
      className={cn(
        'relative h-2 w-full overflow-hidden rounded-full bg-slate-800/80',
        className
      )}
      {...props}
    >
      <div
        className="h-full bg-gradient-to-r from-accent-500 to-plasma-400 transition-all duration-300 ease-out"
        style={{ width: `${percentage}%` }}
      />
    </div>
  );
}

Progress.displayName = 'Progress';
