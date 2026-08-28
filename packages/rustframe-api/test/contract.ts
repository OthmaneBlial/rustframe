import {
  RustFrameError,
  getRustFrame,
  type DatabaseChangeEvent,
  type FilesystemChangeEvent,
  type RustFrameClient,
} from "../src/index.js";

interface DocumentRecord {
  id: number;
  createdAt: string;
  updatedAt: string;
  title: string;
}

interface DocumentInsert {
  title: string;
}

type Tables = {
  documents: {
    record: DocumentRecord;
    insert: DocumentInsert;
    update: Partial<DocumentInsert>;
  };
};

declare const client: RustFrameClient<Tables>;

async function compilePublicContract(): Promise<void> {
  const runtime = getRustFrame<Tables>();
  const records: DocumentRecord[] = await runtime.db.list("documents", { limit: 10 });
  const batch = await client.db.batch([
    { operation: "insert", table: "documents", record: { title: "One" } },
    { operation: "delete", table: "documents", id: 1 },
  ] as const);
  const inserted: DocumentRecord = batch[0];
  const deleted: boolean = batch[1];
  const watcher = await client.fs.watch("grant://workspace", { recursive: true });
  await client.fs.unwatch(watcher.id);
  await client.fs.copyFrom("grant://source", "grant://destination/copied.md");
  const selection = await client.dialog.openDirectory({ title: "Choose workspace" });
  if (selection) await client.fs.listDir(selection.uri);
  client.events.onDatabaseChange((event: DatabaseChangeEvent) => event.tables);
  client.events.onFilesystemChange((event: FilesystemChangeEvent) => event.uri);
  client.events.onFileDrop(({ files }) => files.map((file) => file.uri));
  client.app.openedFiles().map((file) => file.uri);
  client.app.onOpenFiles(({ files }) => files.map((file) => file.name));
  await client.window.open({ id: "reader-1", route: "/reader", title: "Reader" });
  await client.clipboard.writeText(records.at(0)?.title ?? "");
  void inserted;
  void deleted;
}

void compilePublicContract;
void new RustFrameError("permission_denied", "Denied");
