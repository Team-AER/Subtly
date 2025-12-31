import React, { useEffect, useState, useCallback } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Progress } from '@/components/ui/progress';
import { ProgressModal } from '@/components/ui/progress-modal';
import { useRuntimeStore } from '@/state/store';

function formatBytes(bytes) {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function ModelCard({ model, isInstalled, isDownloading, downloadProgress, onDownload, onDelete, isSelected, onSelect }) {
  const progressPercent = downloadProgress?.progress || 0;
  const downloadedBytes = downloadProgress?.downloadedBytes || 0;

  const handleCardClick = () => {
    if (isInstalled) {
      onSelect(model.id);
    }
  };

  return (
    <div
      role={isInstalled ? 'button' : undefined}
      tabIndex={isInstalled ? 0 : undefined}
      onClick={handleCardClick}
      onKeyDown={(e) => {
        if (isInstalled && (e.key === 'Enter' || e.key === ' ')) {
          e.preventDefault();
          onSelect(model.id);
        }
      }}
      className={`relative rounded-xl border p-4 text-left transition ${
        isSelected
          ? 'border-accent-400/80 bg-slate-950/80'
          : isInstalled
          ? 'border-slate-500/30 bg-slate-900/60 hover:border-slate-300/60 cursor-pointer'
          : 'border-slate-600/20 bg-slate-900/40'
      }`}
    >
      {model.recommended && (
        <span className="absolute right-3 top-3 rounded-full bg-accent-500/20 px-2 py-0.5 text-xs font-medium text-accent-400">
          Recommended
        </span>
      )}
      <div className="flex items-start justify-between gap-4">
        <div className="flex-1">
          <h3 className="font-semibold text-slate-100">{model.name}</h3>
          <p className="mt-1 text-xs text-slate-400">{model.description}</p>
          <p className="mt-2 text-xs text-slate-500">{model.size}</p>
        </div>
      </div>

      {isDownloading && (
        <div className="mt-3 space-y-1">
          <Progress value={progressPercent} max={100} showStripes size="lg" />
          <p className="text-xs text-slate-400">
            {formatBytes(downloadedBytes)} / {model.size} ({progressPercent}%)
          </p>
        </div>
      )}

      <div className="mt-3 flex gap-2">
        {isInstalled ? (
          <>
            {isSelected && (
              <span className="flex items-center gap-1 text-xs text-accent-400">
                <svg className="h-3 w-3" fill="currentColor" viewBox="0 0 20 20">
                  <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                </svg>
                Selected
              </span>
            )}
            <Button
              variant="ghost"
              className="ml-auto text-xs text-red-400 hover:text-red-300"
              onClick={(e) => {
                e.stopPropagation();
                onDelete(model.id);
              }}
            >
              Delete
            </Button>
          </>
        ) : isDownloading ? (
          <Button
            variant="secondary"
            className="text-xs"
            onClick={(e) => {
              e.stopPropagation();
              window.aerModels?.cancelDownload(model.id);
            }}
          >
            Cancel
          </Button>
        ) : (
          <Button
            variant="secondary"
            className="text-xs"
            onClick={(e) => {
              e.stopPropagation();
              onDownload(model.id);
            }}
          >
            Download
          </Button>
        )}
      </div>
    </div>
  );
}

export default function ModelManager() {
  const queryClient = useQueryClient();
  const selectedModel = useRuntimeStore((state) => state.selectedModel);
  const setSelectedModel = useRuntimeStore((state) => state.setSelectedModel);
  const addLog = useRuntimeStore((state) => state.addLog);

  const [downloadProgress, setDownloadProgress] = useState({});
  const [downloading, setDownloading] = useState(new Set());
  const [activeDownload, setActiveDownload] = useState(null); // Track active modal download
  const [downloadCancelled, setDownloadCancelled] = useState(false);

  const availableQuery = useQuery({
    queryKey: ['models', 'available'],
    queryFn: () => window.aerModels?.listAvailable(),
    enabled: !!window.aerModels
  });

  const installedQuery = useQuery({
    queryKey: ['models', 'installed'],
    queryFn: () => window.aerModels?.listInstalled(),
    enabled: !!window.aerModels,
    refetchInterval: downloading.size > 0 ? 2000 : false
  });

  const installedIds = new Set(
    (installedQuery.data || [])
      .filter((m) => m.complete)
      .map((m) => m.id)
  );

  // Auto-select first installed model if none selected
  useEffect(() => {
    if (!selectedModel && installedIds.size > 0) {
      const firstInstalled = Array.from(installedIds)[0];
      setSelectedModel(firstInstalled);
    }
  }, [selectedModel, installedIds, setSelectedModel]);

  // Listen for download progress
  useEffect(() => {
    if (!window.aerModels) return;

    window.aerModels.onDownloadProgress((progress) => {
      setDownloadProgress((prev) => ({
        ...prev,
        [progress.modelId]: progress
      }));
    });
  }, []);

  const handleCancelDownload = useCallback(() => {
    if (activeDownload && window.aerModels) {
      window.aerModels.cancelDownload(activeDownload);
      setDownloadCancelled(true);
      setActiveDownload(null);
      addLog(`Download cancelled: ${activeDownload}`);
    }
  }, [activeDownload, addLog]);

  const handleDownload = useCallback(async (modelId) => {
    if (!window.aerModels) return;

    // Find model info for display
    const allModels = [...(availableQuery.data?.whisperModels || [])];
    if (availableQuery.data?.vadModel) {
      allModels.push(availableQuery.data.vadModel);
    }
    const modelInfo = allModels.find(m => m.id === modelId);
    const modelName = modelInfo?.name || modelId;

    setDownloading((prev) => new Set(prev).add(modelId));
    setActiveDownload(modelId);
    setDownloadCancelled(false);
    addLog(`Starting download: ${modelId}`);

    try {
      const result = await window.aerModels.download(modelId);
      if (result.success) {
        addLog(`Downloaded: ${modelId}`);
        queryClient.invalidateQueries({ queryKey: ['models', 'installed'] });
        // Auto-select if it's a whisper model and none selected
        if (modelId !== 'silero-vad' && !selectedModel) {
          setSelectedModel(modelId);
        }
      } else {
        if (!downloadCancelled) {
          addLog(`Download failed: ${result.error}`);
        }
      }
    } catch (err) {
      if (!downloadCancelled) {
        addLog(`Download error: ${err.message}`);
      }
    } finally {
      setDownloading((prev) => {
        const next = new Set(prev);
        next.delete(modelId);
        return next;
      });
      setDownloadProgress((prev) => {
        const next = { ...prev };
        delete next[modelId];
        return next;
      });
      setActiveDownload(null);
    }
  }, [addLog, queryClient, selectedModel, setSelectedModel, availableQuery.data, downloadCancelled]);

  const handleDelete = useCallback(async (modelId) => {
    if (!window.aerModels) return;

    try {
      const result = await window.aerModels.deleteModel(modelId);
      if (result.success) {
        addLog(`Deleted model: ${modelId}`);
        queryClient.invalidateQueries({ queryKey: ['models', 'installed'] });
        if (selectedModel === modelId) {
          setSelectedModel(null);
        }
      } else {
        addLog(`Delete failed: ${result.error}`);
      }
    } catch (err) {
      addLog(`Delete error: ${err.message}`);
    }
  }, [addLog, queryClient, selectedModel, setSelectedModel]);

  const whisperModels = availableQuery.data?.whisperModels || [];
  const vadModel = availableQuery.data?.vadModel;
  const vadInstalled = installedIds.has('silero-vad');

  // Get active download model info for modal
  const activeDownloadInfo = activeDownload ? 
    [...whisperModels, vadModel].find(m => m?.id === activeDownload) : null;
  const activeDownloadProgress = activeDownload ? downloadProgress[activeDownload] : null;

  return (
    <>
      {/* Download Progress Modal */}
      <ProgressModal
        isOpen={!!activeDownload}
        title="Downloading Model"
        description={activeDownloadInfo?.name || activeDownload}
        progress={activeDownloadProgress?.progress || 0}
        currentBytes={activeDownloadProgress?.downloadedBytes}
        totalBytes={activeDownloadProgress?.totalBytes}
        statusMessage={`Downloading ${activeDownloadInfo?.size || 'model'}...`}
        canCancel={true}
        onCancel={handleCancelDownload}
      />

      <Card>
        <CardHeader>
          <CardTitle>Whisper Models</CardTitle>
          <p className="text-xs text-slate-400">
            Select a model for transcription. Larger models are more accurate but slower.
          </p>
        </CardHeader>
        <CardContent>
          <div className="space-y-6">
            {/* VAD Model Section */}
            {vadModel && (
              <div className="rounded-xl border border-slate-600/30 bg-slate-900/40 p-4">
                <div className="flex items-center justify-between">
                  <div>
                    <h4 className="text-sm font-medium text-slate-200">{vadModel.name}</h4>
                    <p className="text-xs text-slate-400">{vadModel.description}</p>
                    <p className="mt-1 text-xs text-slate-500">{vadModel.size}</p>
                  </div>
                {vadInstalled ? (
                  <span className="flex items-center gap-1 text-xs text-emerald-400">
                    <svg className="h-3 w-3" fill="currentColor" viewBox="0 0 20 20">
                      <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                    </svg>
                    Installed
                  </span>
                ) : downloading.has('silero-vad') ? (
                  <div className="w-32">
                    <Progress value={downloadProgress['silero-vad']?.progress || 0} max={100} showStripes />
                    <p className="mt-1 text-xs text-slate-400">
                      {downloadProgress['silero-vad']?.progress || 0}%
                    </p>
                  </div>
                ) : (
                  <Button
                    variant="secondary"
                    className="text-xs"
                    onClick={() => handleDownload('silero-vad')}
                  >
                    Download
                  </Button>
                )}
              </div>
            </div>
          )}

          {/* Whisper Models Grid */}
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
            {whisperModels.map((model) => (
              <ModelCard
                key={model.id}
                model={model}
                isInstalled={installedIds.has(model.id)}
                isDownloading={downloading.has(model.id)}
                downloadProgress={downloadProgress[model.id]}
                onDownload={handleDownload}
                onDelete={handleDelete}
                isSelected={selectedModel === model.id}
                onSelect={setSelectedModel}
              />
            ))}
          </div>

          {!vadInstalled && (
            <p className="text-xs text-amber-400">
              ⚠ VAD model is required for transcription. Please download it first.
            </p>
          )}
        </div>
      </CardContent>
    </Card>
    </>
  );
}
