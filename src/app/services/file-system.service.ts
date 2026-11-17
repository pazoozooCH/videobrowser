import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { FileEntry } from '../models/file-node.model';
import { FrameMode, Preview, VideoFrame, VideoInfo } from '../models/video-frame.model';
import { CacheInfo, GenerateProgress, SyncStats } from '../models/cache.model';

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

  async openInVlc(path: string, startTime?: number): Promise<void> {
    return invoke<void>('open_in_vlc', { path, startTime: startTime ?? null });
  }

  async copyToClipboard(text: string): Promise<void> {
    return invoke<void>('copy_to_clipboard', { text });
  }

  async searchFiles(path: string, pattern: string): Promise<FileEntry[]> {
    return invoke<FileEntry[]>('search_files', { path, pattern });
  }

  async listVideoFiles(path: string): Promise<string[]> {
    return invoke<string[]>('list_video_files', { path });
  }

  async getVideoInfo(path: string): Promise<VideoInfo> {
    return invoke<VideoInfo>('get_video_info', { path });
  }

  async getPreview(path: string, mode: FrameMode): Promise<Preview> {
    return invoke<Preview>('get_preview', { path, mode });
  }

  async extractVideoFrame(path: string, mode: FrameMode, index: number): Promise<VideoFrame> {
    return invoke<VideoFrame>('extract_video_frame', { path, mode, index });
  }

  async syncCache(root: string, full: boolean): Promise<SyncStats> {
    return invoke<SyncStats>('sync_cache', { root, full });
  }

  async generateCache(root: string, modes: FrameMode[], force: boolean): Promise<GenerateProgress> {
    return invoke<GenerateProgress>('generate_cache', { root, modes, force });
  }

  async cancelCacheJob(): Promise<void> {
    return invoke<void>('cancel_cache_job');
  }

  async cacheJobRunning(): Promise<boolean> {
    return invoke<boolean>('cache_job_running');
  }

  async cacheInfo(root: string | null, modes: FrameMode[]): Promise<CacheInfo> {
    return invoke<CacheInfo>('cache_info', { root, modes });
  }

  async clearCache(root: string | null, modes: FrameMode[] | null): Promise<number> {
    return invoke<number>('clear_cache', { root, modes });
  }

  async pickFolder(): Promise<string | null> {
    const selected = await open({ directory: true, multiple: false });
    return selected;
  }
}
