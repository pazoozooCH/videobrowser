import { Component, HostListener, inject } from '@angular/core';
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
  host: { tabindex: '0', style: 'outline: none;' },
})
export class FileTreeComponent {
  protected readonly fileTreeService = inject(FileTreeService);
  private readonly fs = inject(FileSystemService);
  private readonly contextMenuService = inject(ContextMenuService);

  onContextMenu(event: MouseEvent, node: FileTreeNode): void {
    this.fileTreeService.selectedPath.set(node.entry.path);
    this.contextMenuService.show(event, node);
  }

  onNodeClick(node: FileTreeNode): void {
    this.fileTreeService.selectedPath.set(node.entry.path);
    if (node.entry.isDirectory) {
      this.fileTreeService.toggleNode(node);
    }
  }

  onNodeDblClick(node: FileTreeNode): void {
    if (!node.entry.isDirectory) {
      this.fs.openInVlc(node.entry.path);
    }
  }

  isSelected(node: FileTreeNode): boolean {
    return this.fileTreeService.selectedPath() === node.entry.path;
  }

  @HostListener('keydown', ['$event'])
  onKeydown(event: KeyboardEvent): void {
    const nodes = this.fileTreeService.visibleNodes();
    if (!nodes.length) return;

    const selectedPath = this.fileTreeService.selectedPath();
    const selectedIdx = selectedPath
      ? nodes.findIndex((n) => n.entry.path === selectedPath)
      : -1;
    const selected = selectedIdx >= 0 ? nodes[selectedIdx] : null;

    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        if (selectedIdx < nodes.length - 1) {
          this.fileTreeService.selectedPath.set(nodes[selectedIdx + 1].entry.path);
        }
        break;

      case 'ArrowUp':
        event.preventDefault();
        if (selectedIdx > 0) {
          this.fileTreeService.selectedPath.set(nodes[selectedIdx - 1].entry.path);
        }
        break;

      case 'ArrowRight':
        event.preventDefault();
        if (selected?.entry.isDirectory && !selected.isExpanded) {
          this.fileTreeService.expandNode(selected);
        }
        break;

      case 'ArrowLeft':
        event.preventDefault();
        if (selected?.entry.isDirectory && selected.isExpanded) {
          this.fileTreeService.collapseNode(selected);
        }
        break;

      case 'Enter':
        event.preventDefault();
        if (selected?.entry.isDirectory) {
          this.fileTreeService.toggleNode(selected);
        } else if (selected) {
          this.fs.openInVlc(selected.entry.path);
        }
        break;

      case 'F2':
        event.preventDefault();
        if (selected) {
          this.contextMenuService.openRenameDialog(selected);
        }
        break;

      case 'Delete':
        event.preventDefault();
        if (selected) {
          this.contextMenuService.confirmDelete(selected);
        }
        break;
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
