import { inject, Injectable, signal } from '@angular/core';
import { FileEntry } from '../models/file-node.model';
import { FileSystemService } from './file-system.service';
import { FileTreeService } from './file-tree.service';

@Injectable({ providedIn: 'root' })
export class SearchService {
  readonly results = signal<FileEntry[]>([]);
  readonly searchActive = signal(false);
  readonly query = signal('');

  private readonly fs = inject(FileSystemService);
  private readonly fileTreeService = inject(FileTreeService);

  async search(rootPath: string, pattern: string): Promise<void> {
    this.query.set(pattern);
    if (!pattern.trim()) {
      this.results.set([]);
      return;
    }
    const entries = await this.fs.searchFiles(rootPath, pattern);
    this.results.set(entries);
  }

  clear(): void {
    this.searchActive.set(false);
    this.results.set([]);
    this.query.set('');
  }

  async selectResult(entry: FileEntry): Promise<void> {
    await this.expandParents(entry.path);
    this.fileTreeService.selectedPath.set(entry.path);

    requestAnimationFrame(() => {
      document.querySelector('.tree-node.selected')?.scrollIntoView({ block: 'nearest' });
    });
  }

  private async expandParents(path: string): Promise<void> {
    const root = this.fileTreeService.root();
    if (!root) return;

    const rootPath = root.entry.path;
    if (!path.startsWith(rootPath)) return;

    const relative = path.slice(rootPath.length + 1);
    const parts = relative.split('/');
    parts.pop(); // remove the file itself

    let current = root;
    for (const part of parts) {
      if (!current.children) {
        await this.fileTreeService.expandNode(current);
      }
      if (!current.isExpanded) {
        await this.fileTreeService.expandNode(current);
      }

      const child = current.children?.find(
        (c) => c.entry.name === part || c.entry.physicalName === part,
      );
      if (!child) break;
      current = child;
    }

    if (!current.isExpanded && current.entry.isDirectory) {
      await this.fileTreeService.expandNode(current);
    }
  }
}
