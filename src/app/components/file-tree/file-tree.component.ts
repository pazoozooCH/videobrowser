import { Component, inject, input } from '@angular/core';
import { FileTreeNode } from '../../models/file-node.model';
import { FileTreeService } from '../../services/file-tree.service';

@Component({
  selector: 'app-file-tree',
  standalone: true,
  imports: [],
  templateUrl: './file-tree.component.html',
  styleUrl: './file-tree.component.css',
})
export class FileTreeComponent {
  nodes = input.required<FileTreeNode[]>();

  private readonly fileTreeService = inject(FileTreeService);

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

  getIcon(node: FileTreeNode): string {
    if (!node.entry.isDirectory) return '\uD83D\uDCC4';
    return node.isExpanded ? '\uD83D\uDCC2' : '\uD83D\uDCC1';
  }
}
