import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { FileTreeNode } from '../../models/file-node.model';
import { FileTreeService } from '../../services/file-tree.service';

@Component({
  selector: 'app-rename-dialog',
  standalone: true,
  imports: [FormsModule],
  templateUrl: './rename-dialog.component.html',
  styleUrl: './rename-dialog.component.css',
})
export class RenameDialogComponent {
  readonly visible = signal(false);
  readonly currentName = signal('');
  readonly newName = signal('');
  readonly encode = signal(true);

  private node: FileTreeNode | null = null;
  private readonly fileTreeService = inject(FileTreeService);

  show(node: FileTreeNode): void {
    this.node = node;
    this.currentName.set(node.entry.name);
    this.newName.set(node.entry.name);
    this.encode.set(true);
    this.visible.set(true);
  }

  hide(): void {
    this.visible.set(false);
    this.node = null;
  }

  async onRename(): Promise<void> {
    if (!this.node || !this.newName()) return;
    await this.fileTreeService.renameNode(this.node, this.newName(), this.encode());
    this.hide();
  }

  onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      this.onRename();
    } else if (event.key === 'Escape') {
      this.hide();
    }
  }
}
