import { inject, Injectable, NgZone, signal } from '@angular/core';
import { listen } from '@tauri-apps/api/event';
import { CacheInfo, GenerateProgress, SyncStats } from '../models/cache.model';
import { FRAME_MODE_PRESETS, FrameMode } from '../models/video-frame.model';
import { FileSystemService } from './file-system.service';

/** The global cache functions: sync, generate, cancel, info, and clear. */
@Injectable({ providedIn: 'root' })
export class CacheService {
  private readonly fs = inject(FileSystemService);
  private readonly zone = inject(NgZone);

  readonly panelOpen = signal(false);
  readonly info = signal<CacheInfo | null>(null);
  readonly syncing = signal(false);
  readonly syncProgress = signal<SyncStats | null>(null);
  readonly lastSync = signal<SyncStats | null>(null);
  readonly generating = signal(false);
  readonly generateProgress = signal<GenerateProgress | null>(null);
  readonly error = signal<string | null>(null);

  constructor() {
    // Tauri event callbacks may fire outside Angular's zone. Without Tauri, as in unit tests,
    // there are no events and no job.
    listen<SyncStats>('cache-sync-progress', (e) =>
      this.zone.run(() => this.syncProgress.set(e.payload)),
    ).catch(() => {});
    // Events carry counts only; the final result, with the error messages, must not be
    // replaced by an event that arrives after it.
    listen<GenerateProgress>('cache-generate-progress', (e) =>
      this.zone.run(() => {
        if (this.generating()) this.generateProgress.set(e.payload);
      }),
    ).catch(() => {});
    this.fs
      .cacheJobRunning()
      .then((running) => this.generating.set(running))
      .catch(() => {});
  }

  /** Quick sync when a folder is opened: lists only the directories that changed. */
  syncInBackground(root: string): void {
    this.sync(root, false).catch(() => {});
  }

  async sync(root: string, full: boolean): Promise<void> {
    this.syncing.set(true);
    this.error.set(null);
    try {
      this.lastSync.set(await this.fs.syncCache(root, full));
    } catch (e) {
      this.error.set(message(e));
      throw e;
    } finally {
      this.syncing.set(false);
      this.syncProgress.set(null);
      if (this.panelOpen()) await this.refreshInfo(root);
    }
  }

  async generate(root: string, modes: FrameMode[], force: boolean): Promise<void> {
    this.generating.set(true);
    this.error.set(null);
    this.generateProgress.set(null);
    try {
      this.generateProgress.set(await this.fs.generateCache(root, modes, force));
    } catch (e) {
      this.error.set(message(e));
    } finally {
      this.generating.set(false);
      await this.refreshInfo(root);
    }
  }

  async cancel(): Promise<void> {
    await this.fs.cancelCacheJob();
  }

  async clear(root: string | null, refreshRoot: string | null): Promise<void> {
    this.error.set(null);
    try {
      await this.fs.clearCache(root, null);
    } catch (e) {
      this.error.set(message(e));
    }
    await this.refreshInfo(refreshRoot);
  }

  async refreshInfo(root: string | null): Promise<void> {
    try {
      this.info.set(
        await this.fs.cacheInfo(
          root,
          FRAME_MODE_PRESETS.map((p) => p.mode),
        ),
      );
    } catch (e) {
      this.error.set(message(e));
    }
  }
}

function message(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return 'Unknown error';
}
