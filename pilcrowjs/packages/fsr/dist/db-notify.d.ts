import pg from 'pg';
import { FsrStore } from './store.js';
import { FsrWatcher } from './watcher.js';
export declare function startDbNotificationPipeline(connectionString: string, store: FsrStore, watcher: FsrWatcher): Promise<pg.Client>;
//# sourceMappingURL=db-notify.d.ts.map