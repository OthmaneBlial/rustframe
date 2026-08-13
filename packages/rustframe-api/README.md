# rustframe-api

Typed frontend access to the RustFrame desktop runtime.

```ts
import { getRustFrame } from "rustframe-api";
import type { AppRustFrameClient } from "./rustframe.generated";

const rustframe = getRustFrame() as AppRustFrameClient;
const documents = await rustframe.db.list("documents");
```

The package includes runtime availability checks, stable error codes, complete database/filesystem/window/dialog/event types, and global `Window.RustFrame` augmentation. Run `rustframe codegen` in an application to generate its record, insert, update, table-map, and client types from `data/schema.json`.

The injected `window.RustFrame` global remains available to plain JavaScript projects. Native permissions are always enforced inside RustFrame IPC regardless of how the frontend accesses the bridge.
