export interface VideoFrame {
  index: number;
  timestampSecs: number;
  dataBase64: string;
}

export interface VideoInfo {
  durationSecs: number;
  fileSizeBytes: number;
  width: number | null;
  height: number | null;
  displayAspectRatio: string | null;
  codec: string | null;
  bitrate: number | null;
  framerate: string | null;
}

export type FrameMode =
  | { type: 'fixed'; count: number }
  | { type: 'interval'; minutes: number };

export interface FolderVideoEntry {
  filePath: string;
  relativePath: string;
  info: VideoInfo | null;
  frames: VideoFrame[];
  error: string | null;
}
