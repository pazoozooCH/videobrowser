import { inject, Injectable } from '@angular/core';
import { ask, message } from '@tauri-apps/plugin-dialog';
import { FileTreeNode } from '../models/file-node.model';
import { ContextMenuComponent, ContextMenuItem } from '../components/context-menu/context-menu.component';
import { RenameDialogComponent } from '../components/rename-dialog/rename-dialog.component';
import { FileTreeService } from './file-tree.service';
import { FileSystemService } from './file-system.service';
import { PreviewService } from './preview.service';

const VIDEO_EXTENSIONS = ['mp4', 'mkv', 'avi', 'webm', 'mov', 'mpg', 'mpeg'];

@Injectable({ providedIn: 'root' })
export class ContextMenuService {
  private readonly fileTreeService = inject(FileTreeService);
  private readonly fs = inject(FileSystemService);
  private readonly previewService = inject(PreviewService);

  private menuComponent: ContextMenuComponent | null = null;
  private renameDialog: RenameDialogComponent | null = null;

  register(component: ContextMenuComponent): void {
    this.menuComponent = component;
  }

  registerRenameDialog(dialog: RenameDialogComponent): void {
    this.renameDialog = dialog;
  }

  async show(event: MouseEvent, node: FileTreeNode): Promise<void> {
    event.preventDefault();
    event.stopPropagation();

    if (!this.menuComponent) return;

    const canEncode = await this.fs.canEncode(node.entry.path);
    const encodable = node.entry.isDirectory || !node.entry.isEncoded;
    const decodable = node.entry.isDirectory || node.entry.isEncoded;
    const moveSource = this.fileTreeService.moveSource();
    const canMoveHere = this.fileTreeService.canMoveTo(node);

    const ext = node.entry.name.split('.').pop()?.toLowerCase() ?? '';
    const isVideo = !node.entry.isDirectory && VIDEO_EXTENSIONS.includes(ext);

    const items: ContextMenuItem[] = [
      {
        label: 'Open in VLC Player',
        enabled: !node.entry.isDirectory,
        action: () => this.fs.openInVlc(node.entry.path),
      },
      {
        label: 'Video Preview',
        enabled: isVideo || node.entry.isDirectory,
        action: () => node.entry.isDirectory
          ? this.previewService.generateFolderFrames(node.entry.path)
          : this.previewService.generateFrames(node.entry.path),
      },
      {
        label: 'Show in File Manager',
        enabled: true,
        action: () => this.fs.showInFileManager(node.entry.path),
      },
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
      {
        label: 'Rename',
        enabled: true,
        action: () => this.renameDialog?.show(node),
      },
      { label: '', enabled: false, separator: true, action: () => {} },
      {
        label: 'Delete',
        enabled: true,
        action: () => this.confirmDelete(node),
      },
      { label: '', enabled: false, separator: true, action: () => {} },
      {
        label: 'Refresh',
        enabled: node.entry.isDirectory,
        action: () => this.fileTreeService.refreshNode(node),
      },
      { label: '', enabled: false, separator: true, action: () => {} },
      {
        label: 'Select for Move',
        enabled: true,
        action: () => this.fileTreeService.selectForMove(node),
      },
      {
        label: moveSource
          ? `Move [${moveSource.entry.name}] here`
          : 'Select for Move first',
        enabled: canMoveHere,
        action: () => this.fileTreeService.moveNode(node),
      },
    ];

    this.menuComponent.show(event.clientX, event.clientY, items);
  }

  openRenameDialog(node: FileTreeNode): void {
    this.renameDialog?.show(node);
  }

  async confirmDelete(node: FileTreeNode): Promise<void> {
    const childCount = node.entry.isDirectory
      ? await this.fs.countChildren(node.entry.path)
      : 0;

    if (childCount > 5) {
      await message('Node has too many children to be deleted', {
        title: 'Cannot delete',
        kind: 'warning',
      });
      return;
    }

    const childInfo = childCount > 0 ? `\nIt has ${childCount} children` : '';
    const confirmed = await ask(`Really delete '${node.entry.name}'?${childInfo}`, {
      title: 'Really delete?',
      kind: 'warning',
    });

    if (confirmed) {
      await this.fileTreeService.deleteNode(node);
    }
  }
}
