import type { PageRoute, LayoutNode } from './manifest.js';
import type { PilcrowRequest, PilcrowResponse, PilcrowConfig, ServerAdapter } from '@pilcrowjs/core';
export declare function buildPageHandler(module: any, pageMeta: PageRoute, layouts: LayoutNode[], config: PilcrowConfig): (req: PilcrowRequest, res: PilcrowResponse) => Promise<void>;
export declare function buildActionHandler(actions: Record<string, any>): (req: PilcrowRequest, res: PilcrowResponse) => Promise<void>;
export declare function startPilcrow(adapter: ServerAdapter, config: PilcrowConfig, pagesDir: string): Promise<import("./manifest.js").RouteManifest>;
//# sourceMappingURL=boot.d.ts.map