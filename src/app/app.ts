import { Component, inject, viewChild } from '@angular/core';
import { ToolbarComponent } from './components/toolbar/toolbar.component';
import { FileTreeComponent } from './components/file-tree/file-tree.component';
import { ContextMenuComponent } from './components/context-menu/context-menu.component';
import { RenameDialogComponent } from './components/rename-dialog/rename-dialog.component';
import { FileTreeService } from './services/file-tree.service';
import { ContextMenuService } from './services/context-menu.service';

@Component({
  selector: 'app-root',
  imports: [ToolbarComponent, FileTreeComponent, ContextMenuComponent, RenameDialogComponent],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App {
  protected readonly fileTreeService = inject(FileTreeService);
  private readonly contextMenuService = inject(ContextMenuService);
  private readonly contextMenu = viewChild.required(ContextMenuComponent);
  private readonly renameDialog = viewChild.required(RenameDialogComponent);

  ngAfterViewInit(): void {
    this.contextMenuService.register(this.contextMenu());
    this.contextMenuService.registerRenameDialog(this.renameDialog());
  }
}
