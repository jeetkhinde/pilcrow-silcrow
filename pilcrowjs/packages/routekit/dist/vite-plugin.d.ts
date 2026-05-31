import type { Plugin } from 'vite';
export interface PilcrowVitePluginOptions {
    pagesDir: string;
    onRoutesChanged?: () => void | Promise<void>;
}
export declare function pilcrowVitePlugin(options: PilcrowVitePluginOptions): Plugin;
//# sourceMappingURL=vite-plugin.d.ts.map