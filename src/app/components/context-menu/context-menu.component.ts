import { Component, ElementRef, HostListener, signal, viewChild } from '@angular/core';

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
  private readonly menuEl = viewChild<ElementRef<HTMLElement>>('menu');

  readonly visible = signal(false);
  readonly position = signal({ x: 0, y: 0 });
  readonly items = signal<ContextMenuItem[]>([]);

  show(x: number, y: number, items: ContextMenuItem[]): void {
    this.position.set({ x, y });
    this.items.set(items);
    this.visible.set(true);

    requestAnimationFrame(() => {
      const el = this.menuEl()?.nativeElement;
      if (!el) return;

      const rect = el.getBoundingClientRect();
      const vw = window.innerWidth;
      const vh = window.innerHeight;

      let adjustedX = x;
      let adjustedY = y;

      if (rect.bottom > vh) {
        adjustedY = y - rect.height;
      }
      if (rect.right > vw) {
        adjustedX = x - rect.width;
      }
      if (adjustedX < 0) adjustedX = 0;
      if (adjustedY < 0) adjustedY = 0;

      this.position.set({ x: adjustedX, y: adjustedY });
    });
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
