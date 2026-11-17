import { FrameMode } from './video-frame.model';

export interface SyncStats {
  full: boolean;
  dirs: number;
  listedDirs: number;
  entries: number;
  added: number;
  changed: number;
  removed: number;
  reusedVideos: number;
  prunedVideos: number;
  done: boolean;
}

export interface GenerateProgress {
  videosTotal: number;
  videosDone: number;
  framesDone: number;
  framesFailed: number;
  videosFailed: number;
  current: string | null;
  errors: string[];
  done: boolean;
  cancelled: boolean;
}

export interface ModeCoverage {
  mode: FrameMode;
  modeKey: string;
  complete: number;
  failed: number;
}

export interface RootInfo {
  path: string;
  files: number;
  dirs: number;
  videos: number;
  unprobeable: number;
  frames: number;
  frameBytes: number;
  modes: ModeCoverage[];
  lastQuickSync: number | null;
  lastFullSync: number | null;
}

export interface CacheInfo {
  dbPath: string;
  dbBytes: number;
  entries: number;
  videos: number;
  orphanVideos: number;
  frames: number;
  frameBytes: number;
  root: RootInfo | null;
}
