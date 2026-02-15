export interface VideoFrame {
  index: number;
  timestampSecs: number;
  dataBase64: string;
}

export type FrameMode =
  | { type: 'fixed'; count: number }
  | { type: 'interval'; minutes: number };
