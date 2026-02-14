import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { FileEntry } from '../models/file-node.model';

@Injectable({ providedIn: 'root' })
export class FileSystemService {
  async readDirectory(path: string): Promise<FileEntry[]> {
    return invoke<FileEntry[]>('read_directory', { path });
  }

  async encodeNode(path: string): Promise<FileEntry> {
    return invoke<FileEntry>('encode_node', { path });
  }

  async decodeNode(path: string): Promise<FileEntry> {
    return invoke<FileEntry>('decode_node', { path });
  }

  async canEncode(path: string): Promise<boolean> {
    return invoke<boolean>('can_encode_node', { path });
  }

  async renameNode(path: string, newName: string, encode: boolean): Promise<FileEntry> {
    return invoke<FileEntry>('rename_node', { path, newName, encode });
  }

  async copyToClipboard(text: string): Promise<void> {
    return invoke<void>('copy_to_clipboard', { text });
  }

  async pickFolder(): Promise<string | null> {
    const selected = await open({ directory: true, multiple: false });
    return selected;
  }
}
