import type { LiveFieldMeta } from '@pilcrowjs/core';
export interface PageRoute {
    pattern: string;
    filePath: string;
    relativePath: string;
    layouts: string[];
    promoteAfter?: number;
    liveFields: LiveFieldMeta[];
}
export interface LayoutNode {
    filePath: string;
    relativePath: string;
    pattern: string;
}
export interface RouteManifest {
    pages: PageRoute[];
    layouts: LayoutNode[];
    errorPages: Record<string, string>;
    loadingPages: Record<string, string>;
    notFoundPages: Record<string, string>;
}
//# sourceMappingURL=manifest.d.ts.map