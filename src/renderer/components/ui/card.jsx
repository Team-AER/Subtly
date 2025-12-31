import React from 'react';
import { cn } from '@/lib/utils';

export function Card({ className, ...props }) {
  return (
    <div
      className={cn('rounded-2xl border border-slate-500/30 bg-slate-900/70 p-5 shadow-[0_24px_60px_rgba(15,23,42,0.4)] backdrop-blur', className)}
      {...props}
    />
  );
}

export function CardHeader({ className, ...props }) {
  return (
    <div className={cn('flex items-center justify-between gap-4', className)} {...props} />
  );
}

export function CardTitle({ className, ...props }) {
  return (
    <h2 className={cn('font-display text-xl', className)} {...props} />
  );
}

export function CardContent({ className, ...props }) {
  return (
    <div className={cn('mt-4', className)} {...props} />
  );
}
