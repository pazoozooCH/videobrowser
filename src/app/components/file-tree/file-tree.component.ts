import { Component, inject } from '@angular/core';
import { FileTreeNode } from '../../models/file-node.model';
import { FileTreeService } from '../../services/file-tree.service';
import { FileSystemService } from '../../services/file-system.service';
import { ContextMenuService } from '../../services/context-menu.service';

@Component({
  selector: 'app-file-tree',
  standalone: true,
  imports: [],
  templateUrl: './file-tree.component.html',
  styleUrl: './file-tree.component.css',
})
export class FileTreeComponent {
  protected readonly fileTreeService = inject(FileTreeService);
  private readonly fs = inject(FileSystemService);
  private readonly contextMenuService = inject(ContextMenuService);

  onContextMenu(event: MouseEvent, node: FileTreeNode): void {
    this.contextMenuService.show(event, node);
  }

  onNodeClick(node: FileTreeNode): void {
    if (node.entry.isDirectory) {
      this.fileTreeService.toggleNode(node);
    }
  }

  onNodeDblClick(node: FileTreeNode): void {
    if (!node.entry.isDirectory) {
      this.fs.openInVlc(node.entry.path);
    }
  }

  getIndent(node: FileTreeNode): string {
    return `${node.level * 20}px`;
  }

  getArrow(node: FileTreeNode): string {
    if (!node.entry.isDirectory) return '  ';
    return node.isExpanded ? '▾' : '▸';
  }

  getIcon(node: FileTreeNode): string {
    if (!node.entry.isDirectory) return '📄';
    return node.isExpanded ? '📂' : '📁';
  }
}
