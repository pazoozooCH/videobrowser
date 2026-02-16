import { Component, DestroyRef, ElementRef, inject, NgZone, signal, viewChild, effect } from '@angular/core';
import { PreviewService } from '../../services/preview.service';
import { FileSystemService } from '../../services/file-system.service';
import { FrameMode } from '../../models/video-frame.model';

const MIN_PANEL_WIDTH = 400;
const MIN_TREE_WIDTH = 250;

@Component({
  selector: 'app-preview-panel',
  templateUrl: './preview-panel.component.html',
  styleUrl: './preview-panel.component.css',
})
export class PreviewPanelComponent {
  protected readonly preview = inject(PreviewService);
  private readonly fs = inject(FileSystemService);
  private readonly destroyRef = inject(DestroyRef);
  private readonly zone = inject(NgZone);

  protected readonly panelWidth = signal(600);
  private readonly panel = viewChild<ElementRef<HTMLElement>>('panel');
  private readonly scrollContainer = viewChild<ElementRef<HTMLElement>>('folderContent');

  private observer: IntersectionObserver | null = null;
  private observedElements = new Set<Element>();
  protected readonly hiddenEntries = signal(new Set<string>());
  private readonly entryHeights = new Map<string, number>();

  readonly presets: { label: string; mode: FrameMode }[] = [
    { label: '9 frames', mode: { type: 'fixed', count: 9 } },
    { label: '16 frames', mode: { type: 'fixed', count: 16 } },
    { label: 'Every 1 min', mode: { type: 'interval', minutes: 1 } },
    { label: 'Every 5 min', mode: { type: 'interval', minutes: 5 } },
  ];

  constructor() {
    this.destroyRef.onDestroy(() => this.cleanupObserver());

    // React to folder entries changes to observe new elements
    effect(() => {
      const entries = this.preview.folderEntries();
      if (entries.length > 0) {
        // Schedule observation after Angular renders the new elements
        requestAnimationFrame(() => this.observeNewEntries());
      }
    });
  }

  private setupObserver(): void {
    const container = this.scrollContainer()?.nativeElement;
    if (!container || this.observer) return;

    this.observer = new IntersectionObserver(
      (observerEntries) => {
        const hidden = new Set(this.hiddenEntries());
        let changed = false;

        for (const entry of observerEntries) {
          const filePath = (entry.target as HTMLElement).dataset['filePath'];
          if (!filePath) continue;

          if (entry.isIntersecting) {
            if (hidden.has(filePath)) {
              hidden.delete(filePath);
              changed = true;
            }
          } else {
            // Only hide if we have a recorded height (i.e., it was rendered at least once)
            const height = (entry.target as HTMLElement).offsetHeight;
            if (height > 0) {
              this.entryHeights.set(filePath, height);
            }
            if (!hidden.has(filePath) && this.entryHeights.has(filePath)) {
              hidden.add(filePath);
              changed = true;
            }
          }
        }

        if (changed) {
          this.zone.run(() => this.hiddenEntries.set(hidden));
        }
      },
      {
        root: container,
        rootMargin: '400px 0px',
      }
    );
  }

  private observeNewEntries(): void {
    this.setupObserver();
    if (!this.observer) return;
    const container = this.scrollContainer()?.nativeElement;
    if (!container) return;

    const entryElements = container.querySelectorAll('[data-file-path]');
    entryElements.forEach((el) => {
      if (!this.observedElements.has(el)) {
        this.observer!.observe(el);
        this.observedElements.add(el);
      }
    });
  }

  private cleanupObserver(): void {
    this.observer?.disconnect();
    this.observer = null;
    this.observedElements.clear();
  }

  private resetVirtualScroll(): void {
    this.cleanupObserver();
    this.hiddenEntries.set(new Set());
    this.entryHeights.clear();
  }

  isEntryVisible(filePath: string): boolean {
    return !this.hiddenEntries().has(filePath);
  }

  getPlaceholderHeight(filePath: string): number {
    return this.entryHeights.get(filePath) ?? 200;
  }

  selectMode(mode: FrameMode): void {
    this.resetVirtualScroll();
    this.preview.mode.set(mode);
    this.preview.regenerate();
  }

  isActiveMode(mode: FrameMode): boolean {
    const current = this.preview.mode();
    return JSON.stringify(current) === JSON.stringify(mode);
  }

  onResizeStart(event: MouseEvent): void {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = this.panelWidth();
    const mainContent = this.panel()?.nativeElement.closest('.main-content') as HTMLElement | null;
    const availableWidth = mainContent?.clientWidth ?? window.innerWidth;
    const maxWidth = availableWidth - MIN_TREE_WIDTH;

    const onMouseMove = (e: MouseEvent) => {
      const delta = startX - e.clientX;
      const newWidth = Math.max(MIN_PANEL_WIDTH, Math.min(maxWidth, startWidth + delta));
      this.panelWidth.set(newWidth);
    };

    const onMouseUp = () => {
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };

    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }

  formatTimestamp(secs: number): string {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = Math.floor(secs % 60);
    if (h > 0) {
      return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    }
    return `${m}:${s.toString().padStart(2, '0')}`;
  }

  formatFileSize(bytes: number): string {
    if (bytes >= 1_073_741_824) return (bytes / 1_073_741_824).toFixed(2) + ' GB';
    if (bytes >= 1_048_576) return (bytes / 1_048_576).toFixed(1) + ' MB';
    if (bytes >= 1024) return (bytes / 1024).toFixed(0) + ' KB';
    return bytes + ' B';
  }

  formatBitrate(bps: number): string {
    if (bps >= 1_000_000) return (bps / 1_000_000).toFixed(1) + ' Mbps';
    if (bps >= 1_000) return (bps / 1_000).toFixed(0) + ' kbps';
    return bps + ' bps';
  }

  openInVlc(path: string, startTime?: number): void {
    this.fs.openInVlc(path, startTime);
  }
}
