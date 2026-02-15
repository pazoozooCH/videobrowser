export interface FileEntry {
  path: string;
  name: string;
  physicalName: string;
  isDirectory: boolean;
  isEncoded: boolean;
  decodedName: string | null;
  hasChildren: boolean;
  fileSize: number;
}

export interface FileTreeNode {
  entry: FileEntry;
  children: FileTreeNode[] | null;
  isExpanded: boolean;
  level: number;
}
