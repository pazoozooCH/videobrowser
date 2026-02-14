import { Component, HostListener, signal } from '@angular/core';

export interface ContextMenuItem {
  label: string;
  action: () => void;
  enabled: boolean;
  separator?: boolean;
}

@Component({
  selector: 'app-context-menu',
  standalone: true,
  templateUrl: './context-menu.component.html',
  styleUrl: './context-menu.component.css',
})
export class ContextMenuComponent {
  readonly visible = signal(false);
  readonly position = signal({ x: 0, y: 0 });
  readonly items = signal<ContextMenuItem[]>([]);

  show(x: number, y: number, items: ContextMenuItem[]): void {
    this.position.set({ x, y });
    this.items.set(items);
    this.visible.set(true);
  }

  hide(): void {
    this.visible.set(false);
  }

  onItemClick(item: ContextMenuItem): void {
    if (!item.enabled) return;
    this.hide();
    item.action();
  }

  @HostListener('document:click')
  onDocumentClick(): void {
    this.hide();
  }

  @HostListener('document:keydown.escape')
  onEscape(): void {
    this.hide();
  }
}
