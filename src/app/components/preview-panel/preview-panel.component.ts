import { Component, inject } from '@angular/core';
import { PreviewService } from '../../services/preview.service';
import { FrameMode } from '../../models/video-frame.model';

@Component({
  selector: 'app-preview-panel',
  templateUrl: './preview-panel.component.html',
  styleUrl: './preview-panel.component.css',
})
export class PreviewPanelComponent {
  protected readonly preview = inject(PreviewService);

  readonly presets: { label: string; mode: FrameMode }[] = [
    { label: '9 frames', mode: { type: 'fixed', count: 9 } },
    { label: '16 frames', mode: { type: 'fixed', count: 16 } },
    { label: 'Every 1 min', mode: { type: 'interval', minutes: 1 } },
    { label: 'Every 5 min', mode: { type: 'interval', minutes: 5 } },
  ];

  selectMode(mode: FrameMode): void {
    this.preview.mode.set(mode);
    this.preview.regenerate();
  }

  isActiveMode(mode: FrameMode): boolean {
    const current = this.preview.mode();
    return JSON.stringify(current) === JSON.stringify(mode);
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
}
