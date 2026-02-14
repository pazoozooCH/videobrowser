import { inject, Injectable, signal } from '@angular/core';
import { FileEntry, FileTreeNode } from '../models/file-node.model';
import { FileSystemService } from './file-system.service';

@Injectable({ providedIn: 'root' })
export class FileTreeService {
  readonly root = signal<FileTreeNode | null>(null);

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

  private notifyChange(): void {
    // Trigger signal update by re-setting the same root reference
    this.root.set(this.root()!);
  }
}
