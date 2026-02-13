import { Component, inject } from '@angular/core';
import { ToolbarComponent } from './components/toolbar/toolbar.component';
import { FileTreeComponent } from './components/file-tree/file-tree.component';
import { FileTreeService } from './services/file-tree.service';

@Component({
  selector: 'app-root',
  imports: [ToolbarComponent, FileTreeComponent],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App {
  protected readonly fileTreeService = inject(FileTreeService);
}
