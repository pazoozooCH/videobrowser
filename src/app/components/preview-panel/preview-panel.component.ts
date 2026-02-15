import { Component, inject } from '@angular/core';
import { PreviewService } from '../../services/preview.service';
import { FrameMode, VideoFrame } from '../../models/video-frame.model';

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
}
