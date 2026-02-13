import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { FileEntry } from '../models/file-node.model';

@Injectable({ providedIn: 'root' })
export class FileSystemService {
  async readDirectory(path: string): Promise<FileEntry[]> {
    return invoke<FileEntry[]>('read_directory', { path });
  }

  async pickFolder(): Promise<string | null> {
    const selected = await open({ directory: true, multiple: false });
    return selected;
  }
}
