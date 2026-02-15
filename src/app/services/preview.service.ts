import { effect, inject, Injectable, signal } from '@angular/core';
import { FrameMode, VideoFrame, VideoInfo } from '../models/video-frame.model';
import { FileSystemService } from './file-system.service';
import { FileTreeService } from './file-tree.service';

const VIDEO_EXTENSIONS = ['mp4', 'mkv', 'avi', 'webm', 'mov', 'mpg', 'mpeg'];

@Injectable({ providedIn: 'root' })
export class PreviewService {
  private readonly fs = inject(FileSystemService);
  private readonly fileTreeService = inject(FileTreeService);
  private generationId = 0;

  constructor() {
    effect(() => {
      const selectedPath = this.fileTreeService.selectedPath();
      if (!this.active() || !selectedPath || selectedPath === this.currentPath()) return;
      const node = this.fileTreeService.visibleNodes().find(n => n.entry.path === selectedPath);
      if (!node || node.entry.isDirectory) return;
      const ext = node.entry.name.split('.').pop()?.toLowerCase() ?? '';
      if (VIDEO_EXTENSIONS.includes(ext)) {
        this.generateFrames(selectedPath);
      }
    });
  }

  readonly active = signal(false);
  readonly loading = signal(false);
  readonly frames = signal<VideoFrame[]>([]);
  readonly error = signal<string | null>(null);
  readonly currentPath = signal<string | null>(null);
  readonly mode = signal<FrameMode>({ type: 'fixed', count: 9 });
  readonly totalFrames = signal(0);
  readonly info = signal<VideoInfo | null>(null);

  async generateFrames(path: string): Promise<void> {
    const id = ++this.generationId;

    this.active.set(true);
    this.loading.set(true);
    this.error.set(null);
    this.frames.set([]);
    this.currentPath.set(path);
    this.totalFrames.set(0);
    this.info.set(null);

    try {
      const videoInfo = await this.fs.getVideoInfo(path);
      if (id !== this.generationId) return;

      this.info.set(videoInfo);
      const timestamps = this.calculateTimestamps(videoInfo.durationSecs, this.mode());
      this.totalFrames.set(timestamps.length);

      for (let i = 0; i < timestamps.length; i++) {
        if (id !== this.generationId) return;

        const frame = await this.fs.extractVideoFrame(path, timestamps[i], i);
        if (id !== this.generationId) return;

        this.frames.update(prev => [...prev, frame]);
      }
    } catch (e: any) {
      if (id !== this.generationId) return;
      this.error.set(typeof e === 'string' ? e : e.message ?? 'Unknown error');
    } finally {
      if (id === this.generationId) {
        this.loading.set(false);
      }
    }
  }

  async regenerate(): Promise<void> {
    const path = this.currentPath();
    if (path) {
      await this.generateFrames(path);
    }
  }

  close(): void {
    this.generationId++;
    this.active.set(false);
    this.loading.set(false);
    this.frames.set([]);
    this.error.set(null);
    this.currentPath.set(null);
    this.totalFrames.set(0);
    this.info.set(null);
  }

  private calculateTimestamps(duration: number, mode: FrameMode): number[] {
    if (mode.type === 'fixed') {
      if (mode.count === 0) return [];
      const step = duration / (mode.count + 1);
      return Array.from({ length: mode.count }, (_, i) => step * (i + 1));
    } else {
      const interval = mode.minutes * 60;
      const timestamps: number[] = [];
      let t = interval;
      while (t < duration) {
        timestamps.push(t);
        t += interval;
      }
      return timestamps;
    }
  }
}
