import { Component, DestroyRef, inject, viewChild } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { ToolbarComponent } from './components/toolbar/toolbar.component';
import { FileTreeComponent } from './components/file-tree/file-tree.component';
import { ContextMenuComponent } from './components/context-menu/context-menu.component';
import { RenameDialogComponent } from './components/rename-dialog/rename-dialog.component';
import { SearchPanelComponent } from './components/search-panel/search-panel.component';
import { FileTreeService } from './services/file-tree.service';
import { ContextMenuService } from './services/context-menu.service';
import { SearchService } from './services/search.service';

@Component({
  selector: 'app-root',
  imports: [ToolbarComponent, FileTreeComponent, ContextMenuComponent, RenameDialogComponent, SearchPanelComponent],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App {
  protected readonly fileTreeService = inject(FileTreeService);
  private readonly contextMenuService = inject(ContextMenuService);
  private readonly searchService = inject(SearchService);
  private readonly contextMenu = viewChild.required(ContextMenuComponent);
  private readonly renameDialog = viewChild.required(RenameDialogComponent);
  private readonly searchPanel = viewChild(SearchPanelComponent);

  constructor() {
    const onKeydown = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key === 'f') {
        event.preventDefault();
        this.searchService.searchActive.set(true);
        requestAnimationFrame(() => this.searchPanel()?.focusInput());
      }
    };
    document.addEventListener('keydown', onKeydown);
    inject(DestroyRef).onDestroy(() => document.removeEventListener('keydown', onKeydown));
  }

  ngAfterViewInit(): void {
    this.contextMenuService.register(this.contextMenu());
    this.contextMenuService.registerRenameDialog(this.renameDialog());
    this.openCliPath();
  }

  private async openCliPath(): Promise<void> {
    const path = await invoke<string | null>('get_cli_path');
    if (path) {
      await this.fileTreeService.openFolder(path);
    }
  }
}
