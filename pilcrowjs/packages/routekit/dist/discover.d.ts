import type { RouteManifest } from './manifest.js';
export interface RawDiscoveredFile {
    filePath: string;
    relativePath: string;
    dirRelativePath: string;
    fileName: string;
}
export declare function pathToPattern(relativePath: string): string;
export declare function walkDir(dir: string, baseDir?: string): Promise<RawDiscoveredFile[]>;
export declare function discoverRoutes(pagesDir: string): Promise<RouteManifest>;
//# sourceMappingURL=discover.d.ts.map