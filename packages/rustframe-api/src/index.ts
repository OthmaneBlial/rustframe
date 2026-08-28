export type RustFrameErrorCode =
  | "database_unavailable"
  | "database_error"
  | "invalid_request"
  | "invalid_configuration"
  | "invalid_parameter"
  | "ipc_unavailable"
  | "io_error"
  | "missing_assets"
  | "not_found"
  | "permission_denied"
  | "rate_limited"
  | "request_too_large"
  | "time_error"
  | "timeout"
  | "unknown_method"
  | "unknown_error"
  | "webview_error"
  | "window_error";

export type RustFramePermission =
  | "db:read"
  | "db:write"
  | "db:backup"
  | "db:restore"
  | "fs:workspace:read"
  | "fs:workspace:write"
  | "fs:workspace:watch"
  | "fs:grants:read"
  | "fs:grants:write"
  | "fs:grants:watch"
  | "dialog:open"
  | "dialog:save"
  | "window:create"
  | "clipboard:read"
  | "clipboard:write"
  | `shell:${string}`;

export class RustFrameError extends Error {
  readonly code: RustFrameErrorCode;
  readonly details?: unknown;

  constructor(code: RustFrameErrorCode, message: string, details?: unknown) {
    super(message);
    this.name = "RustFrameError";
    this.code = code;
    this.details = details;
  }
}

if (typeof window !== "undefined") {
  window.RustFrameError = RustFrameError;
}

export interface RustFrameTableShape<Record, Insert, Update> {
  record: Record;
  insert: Insert;
  update: Update;
}

export type RustFrameTableMap = Record<string, RustFrameTableShape<object, object, object>>;
export type TableName<Tables extends RustFrameTableMap> = Extract<keyof Tables, string>;

export interface DatabaseFilter {
  field: string;
  op?: "eq" | "ne" | "lt" | "lte" | "gt" | "gte" | "like" | "in";
  value: unknown;
}

export interface DatabaseQuery {
  filters?: DatabaseFilter[];
  orderBy?: Array<{ field: string; direction?: "asc" | "desc" }>;
  limit?: number;
  offset?: number;
}

export type DatabaseBatchOperation<Tables extends RustFrameTableMap> = {
  [Table in TableName<Tables>]:
    | { operation: "insert"; table: Table; record: Tables[Table]["insert"] }
    | { operation: "update"; table: Table; id: number; patch: Tables[Table]["update"] }
    | { operation: "delete"; table: Table; id: number };
}[TableName<Tables>];

export type DatabaseBatchResult<
  Tables extends RustFrameTableMap,
  Operation,
> = Operation extends { operation: "insert" | "update"; table: infer Table }
  ? Table extends TableName<Tables>
    ? Tables[Table]["record"]
    : never
  : Operation extends { operation: "delete" }
    ? boolean
    : never;

export interface RustFrameDatabase<Tables extends RustFrameTableMap> {
  info(): Promise<{ appId: string; dataDir: string; databasePath: string; schemaVersion: number; tables: string[] }>;
  get<Table extends TableName<Tables>>(table: Table, id: number): Promise<Tables[Table]["record"] | null>;
  list<Table extends TableName<Tables>>(table: Table, options?: DatabaseQuery): Promise<Array<Tables[Table]["record"]>>;
  search<Table extends TableName<Tables>>(table: Table, term: string, options?: DatabaseQuery): Promise<Array<Tables[Table]["record"]>>;
  count<Table extends TableName<Tables>>(table: Table, options?: DatabaseQuery): Promise<number>;
  insert<Table extends TableName<Tables>>(table: Table, record: Tables[Table]["insert"]): Promise<Tables[Table]["record"]>;
  update<Table extends TableName<Tables>>(table: Table, id: number, patch: Tables[Table]["update"]): Promise<Tables[Table]["record"]>;
  delete<Table extends TableName<Tables>>(table: Table, id: number): Promise<boolean>;
  batch<const Operations extends readonly DatabaseBatchOperation<Tables>[]>(
    operations: Operations,
  ): Promise<{
    [Index in keyof Operations]: DatabaseBatchResult<Tables, Operations[Index]>;
  }>;
  backup(options?: { suggestedName?: string }): Promise<{ backedUp?: boolean; cancelled: boolean }>;
  restore(): Promise<{ restored: boolean; cancelled?: boolean }>;
}

export interface FilesystemEntry {
  uri: string;
  name: string;
  parent: string;
  isDir: boolean;
  isFile: boolean;
  size: number;
  extension?: string;
  modifiedAt?: string;
}

export interface FilesystemGrant {
  id: string;
  uri: string;
  name: string;
  access: "read" | "read-write";
  kind: "file" | "directory";
  persistent: boolean;
}

export interface RustFrameFilesystem {
  readText(uri: string): Promise<string>;
  readBinary(uri: string): Promise<FilesystemEntry & { byteLength: number; base64: string }>;
  metadata(uri: string): Promise<FilesystemEntry>;
  listDir(uri: string): Promise<FilesystemEntry[]>;
  walk(uri: string, options?: { recursive?: boolean; extensions?: string[]; limit?: number }): Promise<FilesystemEntry[]>;
  writeText(uri: string, contents: string): Promise<FilesystemEntry>;
  writeBinary(uri: string, base64: string): Promise<FilesystemEntry>;
  copyFrom(sourceUri: string, destinationUri: string): Promise<FilesystemEntry>;
  openPath(uri: string): Promise<FilesystemEntry>;
  revealPath(uri: string): Promise<FilesystemEntry>;
  requestGrant(options: { kind: "file" | "directory"; access?: "read" | "read-write"; persist?: boolean; title?: string }): Promise<FilesystemGrant | null>;
  listGrants(): Promise<FilesystemGrant[]>;
  revokeGrant(id: string): Promise<boolean>;
  watch(uri: string, options?: { recursive?: boolean }): Promise<{ id: string }>;
  unwatch(id: string): Promise<boolean>;
}

export interface DatabaseChangeEvent {
  tables: string[];
  operations: Array<"insert" | "update" | "delete" | "restore">;
  recordIds: number[];
  sourceWindowId: string;
}

export interface FilesystemChangeEvent {
  watcherId: string;
  uri: string;
  operation: "create" | "modify" | "rename" | "delete";
  oldUri?: string;
}

export interface FileDropEntry {
  id: string;
  uri: string;
  name: string;
  isDir: boolean;
  isFile: boolean;
  size: number;
  extension?: string;
  modifiedAt?: string;
  access: "read";
  persistent: false;
}

export interface FileOpenEvent {
  files: FileDropEntry[];
}

export interface RustFrameEvents {
  onFileDrop(listener: (event: FileOpenEvent) => void): () => void;
  onDatabaseChange(listener: (event: DatabaseChangeEvent) => void): () => void;
  onFilesystemChange(listener: (event: FilesystemChangeEvent) => void): () => void;
  onRestore(listener: () => void): () => void;
}

export interface RustFrameAppApi {
  /** Files passed by the OS at launch, plus files routed by later app opens. */
  openedFiles(): FileDropEntry[];
  /** Subscribes to files routed after this WebView initialized. */
  onOpenFiles(listener: (event: FileOpenEvent) => void): () => void;
}

export interface RustFrameWindowApi {
  readonly id: string;
  readonly route: string;
  readonly isPrimary: boolean;
  current(): Promise<{ id: string; route: string; isPrimary: boolean; title: string }>;
  list(): Promise<Array<{ id: string; route: string; isPrimary: boolean; title: string }>>;
  open(route: string | { route?: string; id?: string; title?: string; width?: number; height?: number }, options?: { id?: string; title?: string; width?: number; height?: number }): Promise<unknown>;
  close(): Promise<void>;
  minimize(): Promise<void>;
  maximize(): Promise<void>;
  setTitle(title: string): Promise<unknown>;
}

export interface DialogOptions {
  title?: string;
  /** An authorized root:// or grant:// directory URI. */
  directory?: string;
  filters?: Array<{ name: string; extensions: string[] }>;
}

export interface ShellOutput {
  stdout: string;
  stderr: string;
  exitCode: number;
  stdoutTruncated: boolean;
  stderrTruncated: boolean;
  timeoutMs: number;
  maxOutputBytes: number;
}

export interface RustFrameSecurity {
  model: "local-first" | "networked";
  database: boolean;
  filesystem: boolean;
  shell: boolean;
  currentWindow: { id: string; route: string; isPrimary: boolean };
}

export interface RustFrameClient<Tables extends RustFrameTableMap = RustFrameTableMap> {
  app: RustFrameAppApi;
  db: RustFrameDatabase<Tables>;
  fs: RustFrameFilesystem;
  events: RustFrameEvents;
  window: RustFrameWindowApi;
  security: RustFrameSecurity;
  dialog: {
    openFile(options?: DialogOptions): Promise<FilesystemGrant | null>;
    openFiles(options?: DialogOptions): Promise<FilesystemGrant[]>;
    openDirectory(options?: DialogOptions): Promise<FilesystemGrant | null>;
    saveText(options: DialogOptions & { defaultName?: string; contents: string }): Promise<FilesystemGrant | null>;
    saveBinary(options: DialogOptions & { defaultName?: string; base64: string }): Promise<FilesystemGrant | null>;
  };
  clipboard: { readText(): Promise<string>; writeText(text: string): Promise<void> };
  shell: { exec(command: string, args?: string[]): Promise<ShellOutput> };
  path: {
    normalize(path: string): string;
    join(...parts: string[]): string;
    dirname(path: string): string;
    basename(path: string): string;
    extname(path: string): string;
  };
  invoke(method: string, params?: Record<string, unknown>): Promise<unknown>;
}

declare global {
  interface Window {
    RustFrame?: RustFrameClient;
    RustFrameError?: typeof RustFrameError;
  }
}

export function getRustFrame<Tables extends RustFrameTableMap = RustFrameTableMap>(): RustFrameClient<Tables> {
  if (typeof window === "undefined" || !window.RustFrame) {
    throw new RustFrameError("ipc_unavailable", "RustFrame is only available inside the RustFrame desktop runtime");
  }
  return window.RustFrame as unknown as RustFrameClient<Tables>;
}
