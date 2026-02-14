import { Component, inject, input } from '@angular/core';
import { FileTreeNode } from '../../models/file-node.model';
import { FileTreeService } from '../../services/file-tree.service';
import { ContextMenuService } from '../../services/context-menu.service';

@Component({
  selector: 'app-file-tree',
  standalone: true,
  imports: [],
  templateUrl: './file-tree.component.html',
  styleUrl: './file-tree.component.css',
})
export class FileTreeComponent {
  nodes = input.required<FileTreeNode[]>();
  protected readonly revision = inject(FileTreeService).revision;

  private readonly fileTreeService = inject(FileTreeService);
  private readonly contextMenuService = inject(ContextMenuService);

  onContextMenu(event: MouseEvent, node: FileTreeNode): void {
    this.contextMenuService.show(event, node);
  }

  onNodeClick(node: FileTreeNode): void {
    if (node.entry.isDirectory) {
      this.fileTreeService.toggleNode(node);
    }
  }

  getIndent(node: FileTreeNode): string {
    return `${node.level * 20}px`;
  }

  getArrow(node: FileTreeNode): string {
    if (!node.entry.isDirectory) return '  ';
    return node.isExpanded ? '\u25BE' : '\u25B8';
  }

  isExpanded(node: FileTreeNode): boolean {
    this.revision(); // force signal dependency
    return node.isExpanded && node.children !== null;
  }

  getIcon(node: FileTreeNode): string {
    if (!node.entry.isDirectory) return '\uD83D\uDCC4';
    return node.isExpanded ? '\uD83D\uDCC2' : '\uD83D\uDCC1';
  }
}
