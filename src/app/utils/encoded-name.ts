const ENCODING_PREFIX = '.dat_';

/**
 * The name behind a `.dat_` encoded name (base64 of UTF-8), or the name itself if it is not
 * encoded or does not decode. Mirrors `try_decode_name` in the backend.
 */
export function decodeName(name: string): string {
  if (!name.startsWith(ENCODING_PREFIX)) return name;
  try {
    const bytes = Uint8Array.from(atob(name.substring(ENCODING_PREFIX.length)), (c) =>
      c.charCodeAt(0),
    );
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    return name;
  }
}

/** A relative path with every encoded segment decoded, joined with `/`. */
export function decodePath(relativePath: string): string {
  return relativePath
    .split(/[/\\]/)
    .map((segment) => decodeName(segment))
    .join('/');
}
