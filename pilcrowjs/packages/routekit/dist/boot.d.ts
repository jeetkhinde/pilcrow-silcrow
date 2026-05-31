import type { PageRoute, LayoutNode } from './manifest.js';
import type { PilcrowRequest, PilcrowResponse, PilcrowConfig, ServerAdapter } from '@pilcrowjs/core';
/** Insert the FSR client script tag before </head>, or append if no </head>. */
export declare function injectFsrScriptTag(html: string): string;
export declare function buildPageHandler(module: any, pageMeta: PageRoute, layouts: LayoutNode[], config: PilcrowConfig, hasFsr?: boolean): (req: PilcrowRequest, res: PilcrowResponse) => Promise<void>;
export declare function buildActionHandler(actions: Record<string, any>): (req: PilcrowRequest, res: PilcrowResponse) => Promise<void>;
export declare function startPilcrow(adapter: ServerAdapter, config: PilcrowConfig, pagesDir: string, fsr?: {
    store: any;
    watcher: any;
}): Promise<import("./manifest.js").RouteManifest>;
//# sourceMappingURL=boot.d.ts.map