import {
  Component,
  ElementRef,
  afterRenderEffect,
  inject,
  signal,
  viewChildren,
} from '@angular/core';
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

  private readonly textareas = viewChildren<ElementRef<HTMLTextAreaElement>>('autoGrow');

  private node: FileTreeNode | null = null;
  private readonly fileTreeService = inject(FileTreeService);

  constructor() {
    // Auto-grow the textareas so long, wrapped filenames are fully visible.
    // Re-runs whenever the dialog opens or the edited name changes.
    afterRenderEffect(() => {
      this.visible();
      this.newName();
      this.currentName();
      for (const ref of this.textareas()) {
        const el = ref.nativeElement;
        el.style.height = 'auto';
        el.style.height = `${el.scrollHeight}px`;
      }
    });
  }

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

  onNewNameChange(value: string): void {
    // Filenames must not contain newlines; strip any that get pasted in.
    this.newName.set(value.replace(/[\r\n]+/g, ''));
  }

  onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      // Prevent inserting a newline in the textarea; Enter confirms the rename.
      event.preventDefault();
      this.onRename();
    } else if (event.key === 'Escape') {
      this.hide();
    }
  }
}
