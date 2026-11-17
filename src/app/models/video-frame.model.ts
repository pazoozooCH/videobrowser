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

export const FRAME_MODE_PRESETS: { label: string; mode: FrameMode }[] = [
  { label: '9 frames', mode: { type: 'fixed', count: 9 } },
  { label: '18 frames', mode: { type: 'fixed', count: 18 } },
  { label: 'Every 1 min', mode: { type: 'interval', minutes: 1 } },
  { label: 'Every 5 min', mode: { type: 'interval', minutes: 5 } },
];

export function sameMode(a: FrameMode, b: FrameMode): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** What the preview of a video can show at once, and which frames are still to extract. */
export interface Preview {
  info: VideoInfo;
  timestamps: number[];
  frames: VideoFrame[];
  missing: number[];
}

export interface FolderVideoEntry {
  filePath: string;
  relativePath: string;
  decodedRelativePath: string;
  info: VideoInfo | null;
  frames: VideoFrame[];
  error: string | null;
}
