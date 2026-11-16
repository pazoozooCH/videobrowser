import { effect, inject, Injectable, signal } from '@angular/core';
import { FolderVideoEntry, FrameMode, VideoFrame, VideoInfo } from '../models/video-frame.model';
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

  // Folder mode signals
  readonly folderMode = signal(false);
  readonly folderEntries = signal<FolderVideoEntry[]>([]);
  readonly folderPath = signal<string | null>(null);

  async generateFrames(path: string): Promise<void> {
    const id = ++this.generationId;

    this.active.set(true);
    this.loading.set(true);
    this.error.set(null);
    this.frames.set([]);
    this.currentPath.set(path);
    this.totalFrames.set(0);
    this.info.set(null);
    this.folderMode.set(false);
    this.folderEntries.set([]);
    this.folderPath.set(null);

    try {
      await this.loadVideo(path, id, {
        info: (info, total) => {
          this.info.set(info);
          this.totalFrames.set(total);
        },
        frames: (frames) => this.frames.set(frames),
      });
    } catch (e: any) {
      if (id !== this.generationId) return;
      this.error.set(typeof e === 'string' ? e : e.message ?? 'Unknown error');
    } finally {
      if (id === this.generationId) {
        this.loading.set(false);
      }
    }
  }

  async generateFolderFrames(folderPath: string): Promise<void> {
    const id = ++this.generationId;

    this.active.set(true);
    this.loading.set(true);
    this.error.set(null);
    this.frames.set([]);
    this.currentPath.set(null);
    this.info.set(null);
    this.totalFrames.set(0);
    this.folderMode.set(true);
    this.folderEntries.set([]);
    this.folderPath.set(folderPath);

    try {
      const files = await this.fs.listVideoFiles(folderPath);
      if (id !== this.generationId) return;

      if (files.length === 0) {
        this.error.set('No video files found in this folder');
        this.loading.set(false);
        return;
      }

      for (const filePath of files) {
        if (id !== this.generationId) return;

        const relativePath = filePath.startsWith(folderPath)
          ? filePath.substring(folderPath.length).replace(/^[/\\]/, '')
          : filePath;

        const decodedRelativePath = this.decodeRelativePath(relativePath);

        const entry: FolderVideoEntry = {
          filePath,
          relativePath,
          decodedRelativePath,
          info: null,
          frames: [],
          error: null,
        };

        this.folderEntries.update(prev => [...prev, entry]);
        const entryIndex = this.folderEntries().length - 1;

        try {
          await this.loadVideo(filePath, id, {
            info: (info) => this.updateFolderEntry(entryIndex, { info }),
            frames: (frames) => this.updateFolderEntry(entryIndex, { frames }),
          });
        } catch (e: any) {
          if (id !== this.generationId) return;
          this.updateFolderEntry(entryIndex, {
            error: typeof e === 'string' ? e : e.message ?? 'Unknown error',
          });
        }
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

  private updateFolderEntry(index: number, updates: Partial<FolderVideoEntry>): void {
    this.folderEntries.update(entries => {
      const updated = [...entries];
      updated[index] = { ...updated[index], ...updates };
      return updated;
    });
  }

  async regenerate(): Promise<void> {
    if (this.folderMode()) {
      const path = this.folderPath();
      if (path) {
        await this.generateFolderFrames(path);
      }
    } else {
      const path = this.currentPath();
      if (path) {
        await this.generateFrames(path);
      }
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
    this.folderMode.set(false);
    this.folderEntries.set([]);
    this.folderPath.set(null);
  }

  private decodeRelativePath(relativePath: string): string {
    return relativePath.split(/[/\\]/).map(segment => {
      if (segment.startsWith('.dat_')) {
        try {
          return atob(segment.substring(5));
        } catch {
          return segment;
        }
      }
      return segment;
    }).join('/');
  }

  private yield(): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, 0));
  }

  /**
   * Shows what the cache has for a video at once, then extracts the missing frames one by one.
   * Stops when a newer generation started.
   */
  private async loadVideo(
    path: string,
    id: number,
    show: { info: (info: VideoInfo, total: number) => void; frames: (frames: VideoFrame[]) => void },
  ): Promise<void> {
    const mode = this.mode();
    const preview = await this.fs.getPreview(path, mode);
    if (id !== this.generationId) return;

    show.info(preview.info, preview.timestamps.length);
    let frames = preview.frames;
    show.frames(frames);

    for (const index of preview.missing) {
      if (id !== this.generationId) return;
      const frame = await this.fs.extractVideoFrame(path, mode, index);
      if (id !== this.generationId) return;
      frames = [...frames, frame].sort((a, b) => a.index - b.index);
      show.frames(frames);
      await this.yield();
    }
  }
}
