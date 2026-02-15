import { inject, Injectable, signal } from '@angular/core';
import { FrameMode, VideoFrame } from '../models/video-frame.model';
import { FileSystemService } from './file-system.service';

@Injectable({ providedIn: 'root' })
export class PreviewService {
  private readonly fs = inject(FileSystemService);

  readonly active = signal(false);
  readonly loading = signal(false);
  readonly frames = signal<VideoFrame[]>([]);
  readonly error = signal<string | null>(null);
  readonly currentPath = signal<string | null>(null);
  readonly mode = signal<FrameMode>({ type: 'fixed', count: 9 });

  async generateFrames(path: string): Promise<void> {
    this.active.set(true);
    this.loading.set(true);
    this.error.set(null);
    this.frames.set([]);
    this.currentPath.set(path);

    try {
      const frames = await this.fs.generateVideoFrames(path, this.mode());
      this.frames.set(frames);
    } catch (e: any) {
      this.error.set(typeof e === 'string' ? e : e.message ?? 'Unknown error');
    } finally {
      this.loading.set(false);
    }
  }

  async regenerate(): Promise<void> {
    const path = this.currentPath();
    if (path) {
      await this.generateFrames(path);
    }
  }

  close(): void {
    this.active.set(false);
    this.loading.set(false);
    this.frames.set([]);
    this.error.set(null);
    this.currentPath.set(null);
  }
}
