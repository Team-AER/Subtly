import React, { useEffect, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { listDevices, pingRuntime, runSmokeTest, transcribe } from '@/api/runtime';
import { useRuntimeStore } from '@/state/store';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from '@/components/ui/collapsible';
import ModelManager from '@/components/ModelManager';
import { z } from 'zod';

export default function App() {
  const addLog = useRuntimeStore((state) => state.addLog);
  const logs = useRuntimeStore((state) => state.logs);
  const selectedDevice = useRuntimeStore((state) => state.selectedDevice);
  const setSelectedDevice = useRuntimeStore((state) => state.setSelectedDevice);
  const selectedModel = useRuntimeStore((state) => state.selectedModel);
  const inputPath = useRuntimeStore((state) => state.inputPath);
  const outputDir = useRuntimeStore((state) => state.outputDir);
  const settings = useRuntimeStore((state) => state.settings);
  const setInputPath = useRuntimeStore((state) => state.setInputPath);
  const setOutputDir = useRuntimeStore((state) => state.setOutputDir);
  const updateSettings = useRuntimeStore((state) => state.updateSettings);
  const [isTranscribing, setIsTranscribing] = useState(false);
  const [modelPaths, setModelPaths] = useState({ whisper: null, vad: null });

  const pingQuery = useQuery({
    queryKey: ['runtime', 'ping'],
    queryFn: pingRuntime,
    refetchInterval: 10000
  });

  const devicesQuery = useQuery({
    queryKey: ['runtime', 'devices'],
    queryFn: listDevices
  });

  useEffect(() => {
    if (pingQuery.isError) {
      const message = pingQuery.error instanceof Error ? pingQuery.error.message : String(pingQuery.error);
      addLog(`Runtime error: ${message}`);
    }
  }, [pingQuery.isError, pingQuery.error, addLog]);

  // Helper to select the best device: prefer Vulkan/Metal GPU, fallback to CPU
  const selectBestDevice = (devices) => {
    if (!devices || devices.length === 0) return null;
    
    // Priority: Vulkan/Metal discrete GPU > integrated GPU > CPU
    const gpuBackends = ['Vulkan', 'Metal', 'vulkan', 'metal', 'wgpu'];
    const gpuTypes = ['DiscreteGpu', 'IntegratedGpu'];
    
    // Find best GPU device
    for (const gpuType of gpuTypes) {
      const gpu = devices.find(
        (d) => gpuBackends.some((b) => d.backend?.toLowerCase().includes(b.toLowerCase())) && 
               d.device_type === gpuType
      );
      if (gpu) return gpu;
    }
    
    // Fallback to any GPU-capable device
    const anyGpu = devices.find((d) => 
      gpuBackends.some((b) => d.backend?.toLowerCase().includes(b.toLowerCase()))
    );
    if (anyGpu) return anyGpu;
    
    // Final fallback to CPU or first available
    return devices.find((d) => d.device_type === 'Cpu') || devices[0];
  };

  useEffect(() => {
    if (devicesQuery.isSuccess) {
      addLog(`Detected ${devicesQuery.data.devices.length} device(s).`);
      if (!selectedDevice && devicesQuery.data.devices.length > 0) {
        const best = selectBestDevice(devicesQuery.data.devices);
        setSelectedDevice(best);
        if (best) {
          addLog(`Auto-selected device: ${best.name} (${best.backend})`);
        }
      }
    }
    if (devicesQuery.isError) {
      const message = devicesQuery.error instanceof Error ? devicesQuery.error.message : String(devicesQuery.error);
      addLog(`Device query failed: ${message}`);
    }
  }, [devicesQuery.isSuccess, devicesQuery.isError, devicesQuery.data, devicesQuery.error, addLog, selectedDevice, setSelectedDevice]);

  // Resolve model paths when selected model changes
  useEffect(() => {
    async function resolveModelPaths() {
      if (!window.aerModels) return;
      
      const whisperPath = selectedModel 
        ? await window.aerModels.getModelPath(selectedModel)
        : null;
      const vadPath = await window.aerModels.getModelPath('silero-vad');
      
      setModelPaths({ whisper: whisperPath, vad: vadPath });
    }
    resolveModelPaths();
  }, [selectedModel]);

  const handleSmokeTest = async () => {
    addLog('Running Vulkan smoke test...');
    try {
      const result = await runSmokeTest();
      addLog(result.message);
    } catch (err) {
      addLog(`Smoke test failed: ${err.message}`);
    }
  };

  useEffect(() => {
    if (!window.aerRuntime) {
      return undefined;
    }

    const handler = (message) => {
      if (message?.event === 'log') {
        addLog(message.payload);
      }
    };

    window.aerRuntime.onEvent(handler);
    return () => {
      window.aerRuntime.onEvent(() => {});
    };
  }, [addLog]);

  const payloadSchema = z.object({
    input_path: z.string().min(1),
    output_dir: z.string().optional().nullable(),
    model_path: z.string().optional(),
    vad_model_path: z.string().optional(),
    whisper_path: z.string().optional(),
    ffmpeg_path: z.string().optional(),
    vk_icd_filenames: z.string().optional(),
    threads: z.number().int().positive(),
    beam_size: z.number().int().positive(),
    best_of: z.number().int().positive(),
    max_len_chars: z.number().int().positive(),
    split_on_word: z.boolean(),
    vad_threshold: z.number().positive(),
    vad_min_speech_ms: z.number().int().nonnegative(),
    vad_min_sil_ms: z.number().int().nonnegative(),
    vad_pad_ms: z.number().int().nonnegative(),
    no_speech_thold: z.number().positive(),
    max_context: z.number().int().nonnegative(),
    dedup_merge_gap_sec: z.number().positive(),
    translate: z.boolean(),
    language: z.string().min(1),
    dry_run: z.boolean()
  });

  const handleTranscribe = async () => {
    if (!modelPaths.whisper) {
      addLog('Error: No Whisper model selected. Please download and select a model.');
      return;
    }
    if (!modelPaths.vad) {
      addLog('Error: VAD model not installed. Please download the Silero VAD model.');
      return;
    }

    try {
      setIsTranscribing(true);
      const payload = payloadSchema.parse({
        input_path: inputPath,
        output_dir: outputDir || undefined,
        model_path: modelPaths.whisper,
        vad_model_path: modelPaths.vad,
        whisper_path: settings.whisperPath || undefined,
        ffmpeg_path: settings.ffmpegPath || undefined,
        vk_icd_filenames: settings.vkIcdFilenames || undefined,
        threads: Number(settings.threads),
        beam_size: Number(settings.beamSize),
        best_of: Number(settings.bestOf),
        max_len_chars: Number(settings.maxLenChars),
        split_on_word: Boolean(settings.splitOnWord),
        vad_threshold: Number(settings.vadThreshold),
        vad_min_speech_ms: Number(settings.vadMinSpeechMs),
        vad_min_sil_ms: Number(settings.vadMinSilMs),
        vad_pad_ms: Number(settings.vadPadMs),
        no_speech_thold: Number(settings.noSpeechThold),
        max_context: Number(settings.maxContext),
        dedup_merge_gap_sec: Number(settings.dedupMergeGapSec),
        translate: Boolean(settings.translate),
        language: settings.language || 'auto',
        dry_run: Boolean(settings.dryRun)
      });

      addLog('Starting subtitle generation...');
      const result = await transcribe(payload);
      addLog(`Completed ${result.jobs} job(s).`);
      result.outputs.forEach((out) => addLog(`Wrote: ${out}`));
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      addLog(`Transcription failed: ${message}`);
    } finally {
      setIsTranscribing(false);
    }
  };

  const handlePickFile = async () => {
    if (!window.aerDialog) {
      addLog('File picker unavailable.');
      return;
    }
    const path = await window.aerDialog.openFile();
    if (path) {
      setInputPath(path);
    }
  };

  const handlePickDirectory = async () => {
    if (!window.aerDialog) {
      addLog('Directory picker unavailable.');
      return;
    }
    const path = await window.aerDialog.openDirectory();
    if (path) {
      setInputPath(path);
    }
  };

  return (
    <div className="relative min-h-screen overflow-hidden bg-base-950 font-body text-slate-100">
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(30,41,59,0.85)_0%,rgba(11,15,26,0.9)_45%)]" />
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_70%_10%,rgba(56,189,248,0.2)_0%,transparent_55%),radial-gradient(circle_at_10%_80%,rgba(249,115,22,0.2)_0%,transparent_55%)]" />
      <div className="pointer-events-none absolute inset-0 opacity-20 [background-image:linear-gradient(rgba(148,163,184,0.2)_1px,transparent_1px),linear-gradient(90deg,rgba(148,163,184,0.2)_1px,transparent_1px)] [background-size:42px_42px]" />

      <main className="relative mx-auto flex max-w-6xl flex-col gap-8 px-6 pb-20 pt-12">
        <header className="flex flex-wrap items-center justify-between gap-6">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.35em] text-plasma-400">AER subtitles</p>
            <h1 className="font-display text-3xl sm:text-4xl">Whisper subtitle studio</h1>
            <p className="mt-2 text-sm text-slate-300">GPU-accelerated transcription with Vulkan/Metal backends.</p>
          </div>
          <div className="flex items-center gap-3 rounded-2xl border border-slate-500/30 bg-slate-900/80 px-5 py-4">
            <span className={`h-3 w-3 rounded-full ${pingQuery.isSuccess ? 'bg-emerald-400 shadow-[0_0_12px_rgba(52,211,153,0.6)]' : 'bg-orange-400 shadow-[0_0_12px_rgba(249,115,22,0.5)]'}`} />
            <div>
              <p className="text-xs uppercase text-slate-400">Runtime</p>
              <p className="text-sm font-semibold">
                {pingQuery.isSuccess ? pingQuery.data.message : 'Booting...'}
              </p>
              {pingQuery.isSuccess && (
                <p className="text-xs text-slate-400">Backend: {pingQuery.data.backend}</p>
              )}
            </div>
          </div>
        </header>

        <div className="grid gap-6 lg:grid-cols-1">
          <Card>
            <CardHeader>
              <CardTitle>Subtitle workspace</CardTitle>
              <div className="flex gap-2 text-xs text-plasma-400">
                <span className="rounded-full border border-plasma-400/50 px-3 py-1">Whisper</span>
                <span className="rounded-full border border-plasma-400/50 px-3 py-1">VAD</span>
                <span className="rounded-full border border-plasma-400/50 px-3 py-1">SRT output</span>
              </div>
            </CardHeader>
            <CardContent>
              <div className="grid gap-4">
                <div className="grid gap-3 rounded-2xl border border-slate-500/30 bg-slate-900/50 p-4 text-sm text-slate-300">
                  <div className="flex flex-wrap gap-3">
                    <Button variant="secondary" onClick={handlePickFile}>Pick file</Button>
                    <Button variant="secondary" onClick={handlePickDirectory}>Pick folder</Button>
                  </div>
                  <div>
                    <p className="text-xs uppercase text-slate-400">Input path</p>
                    <p className="break-all text-sm text-slate-200">{inputPath || 'No input selected.'}</p>
                  </div>
                </div>
                <label className="grid gap-2 text-sm text-slate-300">
                  Output directory (optional)
                  <input
                    className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                    placeholder="Defaults to input file/folder"
                    value={outputDir}
                    onChange={(event) => setOutputDir(event.target.value)}
                  />
                </label>
                <div className="flex flex-wrap gap-3">
                  <Button 
                    onClick={handleTranscribe} 
                    disabled={isTranscribing || !inputPath || !modelPaths.whisper || !modelPaths.vad}
                  >
                    {isTranscribing ? 'Generating…' : 'Generate subtitles'}
                  </Button>
                  {(!modelPaths.whisper || !modelPaths.vad) && inputPath && (
                    <span className="text-xs text-amber-400 self-center">
                      ⚠ Download models below to enable transcription
                    </span>
                  )}
                </div>
              </div>
            </CardContent>
          </Card>
        </div>

        <ModelManager />

        <Card>
          <Collapsible>
            <CollapsibleTrigger className="w-full">
              <CardHeader className="cursor-pointer">
                <div className="flex w-full items-center justify-between">
                  <CardTitle>Advanced settings</CardTitle>
                  <svg
                    className="h-5 w-5 text-slate-400 transition-transform duration-200 [[data-state=open]_&]:rotate-180"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                  </svg>
                </div>
              </CardHeader>
            </CollapsibleTrigger>
            <CollapsibleContent>
              <CardContent>
                {/* Runtime Telemetry Section */}
                <div className="mb-6">
                  <div className="flex items-center justify-between mb-4">
                    <h3 className="text-sm font-semibold text-slate-200">Runtime telemetry</h3>
                    <Button variant="secondary" size="sm" onClick={handleSmokeTest}>Run Vulkan smoke test</Button>
                  </div>
                  <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4 mb-4">
                    <div className="rounded-xl border border-slate-500/30 bg-slate-900/60 p-3">
                      <p className="text-xs text-slate-400">Queue latency</p>
                      <p className="text-lg font-semibold">--</p>
                    </div>
                    <div className="rounded-xl border border-slate-500/30 bg-slate-900/60 p-3">
                      <p className="text-xs text-slate-400">VRAM usage</p>
                      <p className="text-lg font-semibold">--</p>
                    </div>
                    <div className="rounded-xl border border-slate-500/30 bg-slate-900/60 p-3">
                      <p className="text-xs text-slate-400">Backend</p>
                      <p className="text-lg font-semibold">{pingQuery.data?.backend ?? '--'}</p>
                    </div>
                    <div className="rounded-xl border border-slate-500/30 bg-slate-900/60 p-3">
                      <p className="text-xs text-slate-400">Selected device</p>
                      <p className="text-lg font-semibold">{selectedDevice?.name ?? '--'}</p>
                    </div>
                  </div>
                  <div className="max-h-32 overflow-y-auto rounded-xl border border-dashed border-slate-500/40 bg-slate-950/60 p-3 text-xs text-slate-300 mb-4">
                    {logs.length === 0 ? (
                      <p>No runtime events yet.</p>
                    ) : (
                      logs.map((entry) => <p key={entry.id}>{entry.message}</p>)
                    )}
                  </div>
                </div>

                {/* GPU Devices Section */}
                <div className="mb-6">
                  <div className="flex items-center justify-between mb-4">
                    <h3 className="text-sm font-semibold text-slate-200">GPU devices</h3>
                    <Button variant="secondary" size="sm" onClick={() => devicesQuery.refetch()}>
                      Refresh
                    </Button>
                  </div>
                  <div className="grid gap-3 md:grid-cols-2">
                    {(devicesQuery.data?.devices ?? []).length === 0 && (
                      <div className="rounded-xl border border-slate-500/30 bg-slate-900/60 p-4 text-sm text-slate-300">
                        No compatible GPU devices detected. Using CPU fallback.
                      </div>
                    )}
                    {devicesQuery.data?.devices.map((device) => (
                      <button
                        type="button"
                        key={`${device.name}-${device.device}`}
                        onClick={() => setSelectedDevice(device)}
                        className={`rounded-xl border p-3 text-left transition ${
                          selectedDevice?.name === device.name
                            ? 'border-accent-400/80 bg-slate-950/80'
                            : 'border-slate-500/30 bg-slate-900/60 hover:border-slate-300/60'
                        }`}
                      >
                        <h4 className="font-semibold text-sm">{device.name}</h4>
                        <p className="text-xs text-slate-400">
                          {device.backend} · {device.device_type} · driver {device.driver || 'unknown'}
                        </p>
                      </button>
                    ))}
                  </div>
                </div>

                {/* Whisper Pipeline Settings */}
                <div className="mb-2">
                  <h3 className="text-sm font-semibold text-slate-200 mb-4">Whisper pipeline settings</h3>
                </div>
            <div className="grid gap-4 md:grid-cols-2">
              <label className="grid gap-2 text-sm text-slate-300">
                Whisper CLI path
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  value={settings.whisperPath}
                  placeholder="Auto (bundled in app)"
                  onChange={(event) => updateSettings({ whisperPath: event.target.value })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                FFmpeg path
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  value={settings.ffmpegPath}
                  placeholder="Auto (bundled in app)"
                  onChange={(event) => updateSettings({ ffmpegPath: event.target.value })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                VK_ICD_FILENAMES override (optional)
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  placeholder="/usr/share/vulkan/icd.d/radeon_icd.x86_64.json"
                  value={settings.vkIcdFilenames}
                  onChange={(event) => updateSettings({ vkIcdFilenames: event.target.value })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                Threads
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  type="number"
                  min={1}
                  value={settings.threads}
                  onChange={(event) => updateSettings({ threads: Number(event.target.value) })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                Beam size
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  type="number"
                  min={1}
                  value={settings.beamSize}
                  onChange={(event) => updateSettings({ beamSize: Number(event.target.value) })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                Best of
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  type="number"
                  min={1}
                  value={settings.bestOf}
                  onChange={(event) => updateSettings({ bestOf: Number(event.target.value) })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                Max line length
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  type="number"
                  min={20}
                  value={settings.maxLenChars}
                  onChange={(event) => updateSettings({ maxLenChars: Number(event.target.value) })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                VAD threshold
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  type="number"
                  step="0.01"
                  value={settings.vadThreshold}
                  onChange={(event) => updateSettings({ vadThreshold: Number(event.target.value) })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                VAD min speech (ms)
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  type="number"
                  min={0}
                  value={settings.vadMinSpeechMs}
                  onChange={(event) => updateSettings({ vadMinSpeechMs: Number(event.target.value) })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                VAD min silence (ms)
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  type="number"
                  min={0}
                  value={settings.vadMinSilMs}
                  onChange={(event) => updateSettings({ vadMinSilMs: Number(event.target.value) })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                VAD pad (ms)
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  type="number"
                  min={0}
                  value={settings.vadPadMs}
                  onChange={(event) => updateSettings({ vadPadMs: Number(event.target.value) })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                No speech threshold
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  type="number"
                  step="0.01"
                  value={settings.noSpeechThold}
                  onChange={(event) => updateSettings({ noSpeechThold: Number(event.target.value) })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                Max context
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  type="number"
                  min={0}
                  value={settings.maxContext}
                  onChange={(event) => updateSettings({ maxContext: Number(event.target.value) })}
                />
              </label>
              <label className="flex items-center gap-2 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={settings.splitOnWord}
                  onChange={(event) => updateSettings({ splitOnWord: event.target.checked })}
                />
                Split on word boundaries
              </label>
              <label className="flex items-center gap-2 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={settings.translate}
                  onChange={(event) => updateSettings({ translate: event.target.checked })}
                />
                Translate to English
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                Language
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  value={settings.language}
                  onChange={(event) => updateSettings({ language: event.target.value })}
                />
              </label>
              <label className="grid gap-2 text-sm text-slate-300">
                Dedupe merge gap (sec)
                <input
                  className="rounded-xl border border-slate-500/40 bg-slate-950/70 px-3 py-2 text-sm"
                  type="number"
                  step="0.1"
                  min={0}
                  value={settings.dedupMergeGapSec}
                  onChange={(event) => updateSettings({ dedupMergeGapSec: Number(event.target.value) })}
                />
              </label>
              <label className="flex items-center gap-2 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={settings.dryRun}
                  onChange={(event) => updateSettings({ dryRun: event.target.checked })}
                />
                Dry run
              </label>
            </div>
              </CardContent>
            </CollapsibleContent>
          </Collapsible>
        </Card>
      </main>
    </div>
  );
}
