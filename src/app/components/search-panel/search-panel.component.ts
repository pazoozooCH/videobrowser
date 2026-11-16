import { Component, ElementRef, inject, viewChild } from '@angular/core';
import { SearchService } from '../../services/search.service';
import { FileTreeService } from '../../services/file-tree.service';
import { FileEntry } from '../../models/file-node.model';
import { decodePath } from '../../utils/encoded-name';

@Component({
  selector: 'app-search-panel',
  standalone: true,
  templateUrl: './search-panel.component.html',
  styleUrl: './search-panel.component.css',
})
export class SearchPanelComponent {
  protected readonly searchService = inject(SearchService);
  private readonly fileTreeService = inject(FileTreeService);
  private readonly searchInput = viewChild<ElementRef<HTMLInputElement>>('searchInput');

  focusInput(): void {
    this.searchInput()?.nativeElement.focus();
  }

  onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      this.doSearch();
    } else if (event.key === 'Escape') {
      this.searchService.clear();
    }
  }

  doSearch(): void {
    const root = this.fileTreeService.root();
    if (!root) return;
    const query = this.searchInput()?.nativeElement.value ?? '';
    this.searchService.search(root.entry.path, query);
  }

  selectResult(entry: FileEntry): void {
    this.searchService.selectResult(entry);
  }

  /** The path below the open folder, with encoded names shown decoded. */
  getRelativePath(entry: FileEntry): string {
    const rootPath = this.fileTreeService.root()?.entry.path;
    if (rootPath && entry.path.startsWith(rootPath)) {
      return decodePath(entry.path.slice(rootPath.length).replace(/^[/\\]/, ''));
    }
    return decodePath(entry.path);
  }
}
