import { computed, inject, Injectable, signal } from '@angular/core';
import { FileEntry, FileTreeNode } from '../models/file-node.model';
import { FileSystemService } from './file-system.service';

@Injectable({ providedIn: 'root' })
export class FileTreeService {
  readonly root = signal<FileTreeNode | null>(null);
  readonly moveSource = signal<FileTreeNode | null>(null);

  readonly visibleNodes = computed<FileTreeNode[]>(() => {
    const root = this.root();
    if (!root) return [];
    const result: FileTreeNode[] = [];
    this.collectVisible(root.children ?? [], result);
    return result;
  });

  private readonly fs = inject(FileSystemService);

  async openFolder(path: string): Promise<void> {
    const entries = await this.fs.readDirectory(path);
    const rootEntry: FileEntry = {
      path,
      name: path.split('/').pop() || path,
      physicalName: path.split('/').pop() || path,
      isDirectory: true,
      isEncoded: false,
      decodedName: null,
      hasChildren: entries.length > 0,
    };

    const root: FileTreeNode = {
      entry: rootEntry,
      children: entries.map((e) => this.entryToNode(e, 1)),
      isExpanded: true,
      level: 0,
    };

    this.root.set(root);
  }

  async expandNode(node: FileTreeNode): Promise<void> {
    if (!node.entry.isDirectory || node.isExpanded) return;

    if (node.children === null) {
      const entries = await this.fs.readDirectory(node.entry.path);
      node.children = entries.map((e) => this.entryToNode(e, node.level + 1));
    }

    node.isExpanded = true;
    this.notifyChange();
  }

  collapseNode(node: FileTreeNode): void {
    node.isExpanded = false;
    this.notifyChange();
  }

  toggleNode(node: FileTreeNode): Promise<void> | void {
    if (node.isExpanded) {
      this.collapseNode(node);
    } else {
      return this.expandNode(node);
    }
  }

  async encodeNode(node: FileTreeNode): Promise<void> {
    const updatedEntry = await this.fs.encodeNode(node.entry.path);
    node.entry = updatedEntry;
    if (node.entry.isDirectory && node.isExpanded) {
      node.children = null;
      const entries = await this.fs.readDirectory(node.entry.path);
      node.children = entries.map((e) => this.entryToNode(e, node.level + 1));
    }
    this.notifyChange();
  }

  async decodeNode(node: FileTreeNode): Promise<void> {
    const updatedEntry = await this.fs.decodeNode(node.entry.path);
    node.entry = updatedEntry;
    if (node.entry.isDirectory && node.isExpanded) {
      node.children = null;
      const entries = await this.fs.readDirectory(node.entry.path);
      node.children = entries.map((e) => this.entryToNode(e, node.level + 1));
    }
    this.notifyChange();
  }

  async renameNode(node: FileTreeNode, newName: string, encode: boolean): Promise<void> {
    const updatedEntry = await this.fs.renameNode(node.entry.path, newName, encode);
    node.entry = updatedEntry;
    this.notifyChange();
  }

  async deleteNode(node: FileTreeNode): Promise<void> {
    await this.fs.deleteNode(node.entry.path);
    const parent = this.findParent(node, this.root()!);
    if (parent?.children) {
      parent.children = parent.children.filter((c) => c !== node);
    }
    this.notifyChange();
  }

  selectForMove(node: FileTreeNode): void {
    this.moveSource.set(node);
  }

  canMoveTo(target: FileTreeNode): boolean {
    const source = this.moveSource();
    if (!source || source === target || !target.entry.isDirectory) return false;
    // Can't move into itself or a descendant
    return !this.isDescendantOf(target, source);
  }

  async moveNode(target: FileTreeNode): Promise<void> {
    const source = this.moveSource();
    if (!source || !this.canMoveTo(target)) return;

    await this.fs.moveNode(source.entry.path, target.entry.path);

    // Remove from old parent
    const oldParent = this.findParent(source, this.root()!);
    if (oldParent?.children) {
      oldParent.children = oldParent.children.filter((c) => c !== source);
    }

    // Refresh target to show the moved node
    if (target.children !== null) {
      const entries = await this.fs.readDirectory(target.entry.path);
      target.children = entries.map((e) => this.entryToNode(e, target.level + 1));
    }

    this.moveSource.set(null);
    this.notifyChange();
  }

  private isDescendantOf(node: FileTreeNode, ancestor: FileTreeNode): boolean {
    if (!ancestor.children) return false;
    for (const child of ancestor.children) {
      if (child === node) return true;
      if (this.isDescendantOf(node, child)) return true;
    }
    return false;
  }

  async refreshNode(node: FileTreeNode): Promise<void> {
    if (!node.entry.isDirectory) return;
    node.children = null;
    const entries = await this.fs.readDirectory(node.entry.path);
    node.children = entries.map((e) => this.entryToNode(e, node.level + 1));
    node.isExpanded = true;
    this.notifyChange();
  }

  private entryToNode(entry: FileEntry, level: number): FileTreeNode {
    return {
      entry,
      children: null,
      isExpanded: false,
      level,
    };
  }

  private collectVisible(nodes: FileTreeNode[], result: FileTreeNode[]): void {
    for (const node of nodes) {
      result.push(node);
      if (node.isExpanded && node.children) {
        this.collectVisible(node.children, result);
      }
    }
  }

  private findParent(target: FileTreeNode, current: FileTreeNode): FileTreeNode | null {
    if (current.children) {
      for (const child of current.children) {
        if (child === target) return current;
        const found = this.findParent(target, child);
        if (found) return found;
      }
    }
    return null;
  }

  private notifyChange(): void {
    this.root.set(this.cloneTree(this.root()!));
  }

  private cloneTree(node: FileTreeNode): FileTreeNode {
    return {
      ...node,
      children: node.children?.map((c) => this.cloneTree(c)) ?? null,
    };
  }
}
