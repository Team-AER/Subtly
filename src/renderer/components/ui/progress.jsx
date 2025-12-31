import React from 'react';
import { cn } from '@/lib/utils';

export function Progress({ value = 0, max = 100, className, showStripes = false, size = 'default', ...props }) {
  const percentage = Math.min(100, Math.max(0, (value / max) * 100));
  
  const sizeClasses = {
    sm: 'h-1.5',
    default: 'h-2',
    lg: 'h-3',
  };

  return (
    <div
      className={cn(
        'relative w-full overflow-hidden rounded-full bg-slate-800/80',
        sizeClasses[size] || sizeClasses.default,
        className
      )}
      {...props}
    >
      {showStripes && (
        <div 
          className="absolute inset-0 opacity-30"
          style={{
            backgroundImage: 'repeating-linear-gradient(45deg, transparent, transparent 8px, rgba(255,255,255,0.1) 8px, rgba(255,255,255,0.1) 16px)',
            animation: 'progress-stripes 1s linear infinite',
          }}
        />
      )}
      <div
        className="relative h-full bg-gradient-to-r from-accent-500 via-plasma-400 to-accent-500 transition-all duration-300 ease-out"
        style={{ 
          width: `${percentage}%`,
          backgroundSize: showStripes ? '200% 100%' : undefined,
          animation: showStripes ? 'shimmer 2s ease-in-out infinite' : undefined,
        }}
      >
        {showStripes && (
          <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/10 to-transparent" />
        )}
      </div>
      {showStripes && percentage > 0 && percentage < 100 && (
        <div
          className="absolute top-0 h-full w-0.5 animate-pulse bg-white/50"
          style={{ left: `${percentage}%`, transform: 'translateX(-50%)' }}
        />
      )}
      <style>{`
        @keyframes shimmer {
          0% { background-position: 200% 0; }
          100% { background-position: -200% 0; }
        }
        @keyframes progress-stripes {
          0% { background-position: 0 0; }
          100% { background-position: 32px 0; }
        }
      `}</style>
    </div>
  );
}

Progress.displayName = 'Progress';
