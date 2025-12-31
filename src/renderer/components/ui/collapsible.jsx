import React, { useState } from 'react';

function Collapsible({ children, defaultOpen = false }) {
  const [open, setOpen] = useState(defaultOpen);
  
  return (
    <div data-state={open ? 'open' : 'closed'}>
      {React.Children.map(children, (child) => {
        if (React.isValidElement(child)) {
          return React.cloneElement(child, { open, setOpen });
        }
        return child;
      })}
    </div>
  );
}

function CollapsibleTrigger({ children, open, setOpen, className = '', asChild = false }) {
  const handleClick = () => setOpen?.(!open);
  
  if (asChild && React.isValidElement(children)) {
    return React.cloneElement(children, {
      onClick: handleClick,
      'data-state': open ? 'open' : 'closed',
    });
  }
  
  return (
    <button
      type="button"
      onClick={handleClick}
      className={className}
      data-state={open ? 'open' : 'closed'}
    >
      {children}
    </button>
  );
}

function CollapsibleContent({ children, open, className = '' }) {
  if (!open) return null;
  
  return (
    <div className={className} data-state={open ? 'open' : 'closed'}>
      {children}
    </div>
  );
}

export { Collapsible, CollapsibleTrigger, CollapsibleContent };
