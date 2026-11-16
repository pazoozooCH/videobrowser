import { Component, computed, effect, inject, signal } from '@angular/core';
import { ask } from '@tauri-apps/plugin-dialog';
import { CacheService } from '../../services/cache.service';
import { FileTreeService } from '../../services/file-tree.service';
import { FRAME_MODE_PRESETS, FrameMode, sameMode } from '../../models/video-frame.model';
import { ModeCoverage } from '../../models/cache.model';

@Component({
  selector: 'app-cache-panel',
  standalone: true,
  templateUrl: './cache-panel.component.html',
  styleUrl: './cache-panel.component.css',
})
export class CachePanelComponent {
  protected readonly cache = inject(CacheService);
  private readonly fileTree = inject(FileTreeService);

  protected readonly presets = FRAME_MODE_PRESETS;
  protected readonly selectedModes = signal<FrameMode[]>([FRAME_MODE_PRESETS[0].mode]);
  protected readonly force = signal(false);
  protected readonly rootPath = computed(() => this.fileTree.root()?.entry.path ?? null);
  protected readonly busy = computed(() => this.cache.syncing() || this.cache.generating());

  constructor() {
    // Show the info of the folder that is open whenever the panel opens or the folder changes.
    effect(() => {
      if (this.cache.panelOpen()) {
        this.cache.refreshInfo(this.rootPath());
      }
    });
  }

  isSelected(mode: FrameMode): boolean {
    return this.selectedModes().some((m) => sameMode(m, mode));
  }

  toggleMode(mode: FrameMode): void {
    this.selectedModes.update((modes) =>
      this.isSelected(mode) ? modes.filter((m) => !sameMode(m, mode)) : [...modes, mode],
    );
  }

  label(coverage: ModeCoverage): string {
    return this.presets.find((p) => sameMode(p.mode, coverage.mode))?.label ?? coverage.modeKey;
  }

  percent(done: number, total: number): number {
    return total > 0 ? Math.round((done / total) * 100) : 0;
  }

  sync(full: boolean): void {
    const root = this.rootPath();
    if (root) this.cache.sync(root, full).catch(() => {});
  }

  generate(): void {
    const root = this.rootPath();
    if (root && this.selectedModes().length > 0) {
      this.cache.generate(root, this.selectedModes(), this.force());
    }
  }

  async clear(all: boolean): Promise<void> {
    const root = this.rootPath();
    const question = all
      ? 'Delete all cached frames of every folder?'
      : `Delete the cached frames of the videos below '${root}'?`;
    if (await ask(question, { title: 'Clear cache', kind: 'warning' })) {
      await this.cache.clear(all ? null : root, root);
    }
  }

  formatBytes(bytes: number): string {
    if (bytes >= 1_073_741_824) return (bytes / 1_073_741_824).toFixed(2) + ' GB';
    if (bytes >= 1_048_576) return (bytes / 1_048_576).toFixed(1) + ' MB';
    if (bytes >= 1024) return (bytes / 1024).toFixed(0) + ' KB';
    return bytes + ' B';
  }

  formatAgo(epochSecs: number | null): string {
    if (epochSecs === null) return 'never';
    const secs = Math.max(0, Date.now() / 1000 - epochSecs);
    if (secs < 60) return 'just now';
    if (secs < 3600) return `${Math.floor(secs / 60)} min ago`;
    if (secs < 86_400) return `${Math.floor(secs / 3600)} h ago`;
    return new Date(epochSecs * 1000).toLocaleDateString();
  }
}
