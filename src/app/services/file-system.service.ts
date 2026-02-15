import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { FileEntry } from '../models/file-node.model';
import { VideoFrame } from '../models/video-frame.model';

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

  async deleteNode(path: string): Promise<void> {
    return invoke<void>('delete_node', { path });
  }

  async countChildren(path: string): Promise<number> {
    return invoke<number>('count_children', { path });
  }

  async moveNode(source: string, targetDir: string): Promise<FileEntry> {
    return invoke<FileEntry>('move_node', { source, targetDir });
  }

  async showInFileManager(path: string): Promise<void> {
    return invoke<void>('show_in_file_manager', { path });
  }

  async openInVlc(path: string): Promise<void> {
    return invoke<void>('open_in_vlc', { path });
  }

  async copyToClipboard(text: string): Promise<void> {
    return invoke<void>('copy_to_clipboard', { text });
  }

  async searchFiles(path: string, pattern: string): Promise<FileEntry[]> {
    return invoke<FileEntry[]>('search_files', { path, pattern });
  }

  async getVideoDuration(path: string): Promise<number> {
    return invoke<number>('get_video_duration', { path });
  }

  async extractVideoFrame(path: string, timestampSecs: number, index: number): Promise<VideoFrame> {
    return invoke<VideoFrame>('extract_video_frame', { path, timestampSecs, index });
  }

  async pickFolder(): Promise<string | null> {
    const selected = await open({ directory: true, multiple: false });
    return selected;
  }
}
