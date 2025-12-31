import React from 'react';
import { Slot } from '@radix-ui/react-slot';
import { cn } from '@/lib/utils';

const buttonStyles = {
  base: 'inline-flex items-center justify-center rounded-full px-5 py-2 text-sm font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-orange-400 focus-visible:ring-offset-2 focus-visible:ring-offset-base-950 disabled:pointer-events-none disabled:opacity-50',
  primary: 'bg-gradient-to-br from-accent-500 to-accent-400 text-base-950 shadow-glow',
  secondary: 'border border-slate-500/50 text-slate-100 hover:border-slate-300/70',
  ghost: 'text-slate-200 hover:bg-white/5'
};

export const Button = React.forwardRef(({ className, variant = 'primary', asChild = false, ...props }, ref) => {
  const Comp = asChild ? Slot : 'button';
  return (
    <Comp
      ref={ref}
      className={cn(buttonStyles.base, buttonStyles[variant], className)}
      {...props}
    />
  );
});

Button.displayName = 'Button';
