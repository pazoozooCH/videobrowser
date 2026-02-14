import { inject, Injectable, signal } from '@angular/core';
import { FileTreeNode } from '../models/file-node.model';
import { ContextMenuComponent, ContextMenuItem } from '../components/context-menu/context-menu.component';
import { FileTreeService } from './file-tree.service';
import { FileSystemService } from './file-system.service';

@Injectable({ providedIn: 'root' })
export class ContextMenuService {
  private readonly fileTreeService = inject(FileTreeService);
  private readonly fs = inject(FileSystemService);

  private menuComponent: ContextMenuComponent | null = null;

  register(component: ContextMenuComponent): void {
    this.menuComponent = component;
  }

  async show(event: MouseEvent, node: FileTreeNode): Promise<void> {
    event.preventDefault();
    event.stopPropagation();

    if (!this.menuComponent) return;

    const canEncode = await this.fs.canEncode(node.entry.path);
    const encodable = node.entry.isDirectory || !node.entry.isEncoded;
    const decodable = node.entry.isDirectory || node.entry.isEncoded;

    const items: ContextMenuItem[] = [
      {
        label: 'Copy Path',
        enabled: true,
        action: () => this.fs.copyToClipboard(node.entry.path),
      },
      { label: '', enabled: false, separator: true, action: () => {} },
      {
        label: 'Encode' + (!canEncode ? ' (maybe too long)' : ''),
        enabled: encodable,
        action: () => this.fileTreeService.encodeNode(node),
      },
      {
        label: 'Decode',
        enabled: decodable,
        action: () => this.fileTreeService.decodeNode(node),
      },
      { label: '', enabled: false, separator: true, action: () => {} },
      {
        label: 'Refresh',
        enabled: node.entry.isDirectory,
        action: () => this.fileTreeService.refreshNode(node),
      },
    ];

    this.menuComponent.show(event.clientX, event.clientY, items);
  }
}
