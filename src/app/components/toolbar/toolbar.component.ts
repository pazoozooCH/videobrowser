import { Component, inject } from '@angular/core';
import { FileSystemService } from '../../services/file-system.service';
import { FileTreeService } from '../../services/file-tree.service';
import { SearchService } from '../../services/search.service';
import { PreviewService } from '../../services/preview.service';

@Component({
  selector: 'app-toolbar',
  standalone: true,
  templateUrl: './toolbar.component.html',
  styleUrl: './toolbar.component.css',
})
export class ToolbarComponent {
  private readonly fsService = inject(FileSystemService);
  private readonly fileTreeService = inject(FileTreeService);
  protected readonly searchService = inject(SearchService);
  protected readonly previewService = inject(PreviewService);

  async openFolder(): Promise<void> {
    const path = await this.fsService.pickFolder();
    if (path) {
      await this.fileTreeService.openFolder(path);
    }
  }

  togglePreview(): void {
    if (this.previewService.active()) {
      this.previewService.close();
    } else {
      this.previewService.active.set(true);
    }
  }
}
