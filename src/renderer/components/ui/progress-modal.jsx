import React, { useEffect, useState, useRef } from 'react';
import { cn } from '@/lib/utils';

export function formatBytes(bytes) {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

export function formatTime(seconds) {
  if (!seconds || seconds === Infinity || isNaN(seconds)) return '--:--';
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  if (mins >= 60) {
    const hours = Math.floor(mins / 60);
    const remainingMins = mins % 60;
    return `${hours}h ${remainingMins}m`;
  }
  return `${mins}:${secs.toString().padStart(2, '0')}`;
}

function AnimatedProgress({ value = 0, className }) {
  const percentage = Math.min(100, Math.max(0, value));

  return (
    <div
      className={cn(
        'relative h-3 w-full overflow-hidden rounded-full bg-slate-800/80',
        className
      )}
    >
      {/* Animated background stripes */}
      <div 
        className="absolute inset-0 opacity-30"
        style={{
          backgroundImage: 'repeating-linear-gradient(45deg, transparent, transparent 10px, rgba(255,255,255,0.1) 10px, rgba(255,255,255,0.1) 20px)',
          animation: 'progress-stripes 1s linear infinite',
        }}
      />
      {/* Main progress bar */}
      <div
        className="relative h-full bg-gradient-to-r from-accent-500 via-plasma-400 to-accent-500 transition-all duration-300 ease-out"
        style={{ 
          width: `${percentage}%`,
          backgroundSize: '200% 100%',
          animation: 'shimmer 2s ease-in-out infinite',
        }}
      >
        {/* Glow effect */}
        <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/20 to-transparent" />
      </div>
      {/* Pulse indicator at the end */}
      {percentage > 0 && percentage < 100 && (
        <div
          className="absolute top-0 h-full w-1 animate-pulse bg-white/60"
          style={{ left: `${percentage}%`, transform: 'translateX(-50%)' }}
        />
      )}
    </div>
  );
}

export function ProgressModal({
  isOpen,
  title = 'Processing...',
  description,
  progress = 0,
  currentBytes,
  totalBytes,
  currentItem,
  totalItems,
  onCancel,
  canCancel = true,
  statusMessage,
}) {
  const [eta, setEta] = useState(null);
  const startTimeRef = useRef(null);
  const lastProgressRef = useRef({ progress: 0, time: Date.now(), bytes: 0 });
  const speedSamplesRef = useRef([]);

  useEffect(() => {
    if (isOpen && progress === 0) {
      startTimeRef.current = Date.now();
      lastProgressRef.current = { progress: 0, time: Date.now(), bytes: 0 };
      speedSamplesRef.current = [];
      setEta(null);
    }
  }, [isOpen, progress]);

  // Calculate ETA based on progress
  useEffect(() => {
    if (!isOpen || progress <= 0) return;

    const now = Date.now();
    const lastUpdate = lastProgressRef.current;
    const timeDelta = (now - lastUpdate.time) / 1000;

    if (timeDelta >= 0.5) {
      let speed;
      
      if (currentBytes !== undefined && totalBytes) {
        // Use byte-based speed calculation for downloads
        const bytesDelta = currentBytes - lastUpdate.bytes;
        speed = bytesDelta / timeDelta;
        
        speedSamplesRef.current.push(speed);
        if (speedSamplesRef.current.length > 10) {
          speedSamplesRef.current.shift();
        }
        
        const avgSpeed = speedSamplesRef.current.reduce((a, b) => a + b, 0) / speedSamplesRef.current.length;
        const remainingBytes = totalBytes - currentBytes;
        const estimatedSeconds = avgSpeed > 0 ? remainingBytes / avgSpeed : null;
        setEta(estimatedSeconds);
        
        lastProgressRef.current = { progress, time: now, bytes: currentBytes };
      } else {
        // Use percentage-based calculation for transcription
        const progressDelta = progress - lastUpdate.progress;
        speed = progressDelta / timeDelta;
        
        speedSamplesRef.current.push(speed);
        if (speedSamplesRef.current.length > 10) {
          speedSamplesRef.current.shift();
        }
        
        const avgSpeed = speedSamplesRef.current.reduce((a, b) => a + b, 0) / speedSamplesRef.current.length;
        const remainingProgress = 100 - progress;
        const estimatedSeconds = avgSpeed > 0 ? remainingProgress / avgSpeed : null;
        setEta(estimatedSeconds);
        
        lastProgressRef.current = { progress, time: now, bytes: currentBytes || 0 };
      }
    }
  }, [isOpen, progress, currentBytes, totalBytes]);

  if (!isOpen) return null;

  const progressPercent = Math.round(progress);

  return (
    <>
      {/* CSS for animations */}
      <style>{`
        @keyframes shimmer {
          0% { background-position: 200% 0; }
          100% { background-position: -200% 0; }
        }
        @keyframes progress-stripes {
          0% { background-position: 0 0; }
          100% { background-position: 40px 0; }
        }
        @keyframes pulse-ring {
          0% { transform: scale(0.8); opacity: 0.5; }
          50% { transform: scale(1); opacity: 0.3; }
          100% { transform: scale(0.8); opacity: 0.5; }
        }
      `}</style>
      
      {/* Backdrop - blocks all interaction */}
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
        {/* Modal */}
        <div className="relative mx-4 w-full max-w-md overflow-hidden rounded-2xl border border-slate-500/30 bg-slate-900/95 shadow-2xl">
          {/* Decorative header glow */}
          <div className="absolute left-0 right-0 top-0 h-32 bg-gradient-to-b from-accent-500/10 to-transparent" />
          
          <div className="relative p-6">
            {/* Header */}
            <div className="mb-6 flex items-center gap-4">
              {/* Animated spinner */}
              <div className="relative flex h-12 w-12 items-center justify-center">
                <div className="absolute inset-0 animate-spin rounded-full border-2 border-transparent border-t-accent-400" style={{ animationDuration: '1s' }} />
                <div className="absolute inset-1 animate-spin rounded-full border-2 border-transparent border-t-plasma-400" style={{ animationDuration: '1.5s', animationDirection: 'reverse' }} />
                <div className="absolute inset-2 rounded-full bg-gradient-to-br from-accent-500/20 to-plasma-500/20" style={{ animation: 'pulse-ring 2s ease-in-out infinite' }} />
                <span className="text-lg font-bold text-accent-400">{progressPercent}%</span>
              </div>
              
              <div className="flex-1">
                <h3 className="text-lg font-semibold text-slate-100">{title}</h3>
                {description && (
                  <p className="text-sm text-slate-400 truncate">{description}</p>
                )}
              </div>
            </div>

            {/* Progress bar */}
            <div className="mb-4">
              <AnimatedProgress value={progress} />
            </div>

            {/* Stats section */}
            <div className="mb-4">
              {/* Progress info */}
              <div className="rounded-xl border border-slate-500/20 bg-slate-800/50 p-3">
                <p className="text-xs text-slate-500 uppercase tracking-wide">Progress</p>
                <p className="text-lg font-semibold text-slate-200">
                  {currentBytes !== undefined ? (
                    <>
                      {formatBytes(currentBytes)}
                      <span className="text-slate-500 text-sm"> / {formatBytes(totalBytes || 0)}</span>
                    </>
                  ) : (
                    <>
                      {progressPercent}%
                      {totalItems && currentItem !== undefined && (
                        <span className="text-slate-500 text-sm"> ({currentItem}/{totalItems})</span>
                      )}
                    </>
                  )}
                </p>
              </div>
            </div>

            {/* Status message */}
            {statusMessage && (
              <div className="mb-4 rounded-lg border border-slate-500/20 bg-slate-800/30 px-3 py-2">
                <p className="text-sm text-slate-400 truncate">{statusMessage}</p>
              </div>
            )}

            {/* Cancel button */}
            {canCancel && onCancel && (
              <button
                type="button"
                onClick={onCancel}
                className="w-full rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm font-medium text-red-400 transition-all hover:border-red-500/50 hover:bg-red-500/20 focus:outline-none focus:ring-2 focus:ring-red-500/50"
              >
                Cancel
              </button>
            )}
          </div>
        </div>
      </div>
    </>
  );
}

ProgressModal.displayName = 'ProgressModal';
