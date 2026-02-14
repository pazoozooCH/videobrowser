import { Component, ElementRef, inject, viewChild } from '@angular/core';
import { SearchService } from '../../services/search.service';
import { FileTreeService } from '../../services/file-tree.service';
import { FileEntry } from '../../models/file-node.model';

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

  getRelativePath(entry: FileEntry): string {
    const root = this.fileTreeService.root();
    if (!root) return entry.path;
    const rootPath = root.entry.path;
    if (entry.path.startsWith(rootPath)) {
      return entry.path.slice(rootPath.length + 1);
    }
    return entry.path;
  }
}
