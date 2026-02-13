import { Component, inject } from '@angular/core';
import { FileSystemService } from '../../services/file-system.service';
import { FileTreeService } from '../../services/file-tree.service';

@Component({
  selector: 'app-toolbar',
  standalone: true,
  templateUrl: './toolbar.component.html',
  styleUrl: './toolbar.component.css',
})
export class ToolbarComponent {
  private readonly fsService = inject(FileSystemService);
  private readonly fileTreeService = inject(FileTreeService);

  async openFolder(): Promise<void> {
    const path = await this.fsService.pickFolder();
    if (path) {
      await this.fileTreeService.openFolder(path);
    }
  }
}
