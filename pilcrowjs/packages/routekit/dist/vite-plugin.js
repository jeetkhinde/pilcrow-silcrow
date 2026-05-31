export function pilcrowVitePlugin(options) {
    return {
        name: 'vite-plugin-pilcrow',
        configureServer(server) {
            const handleFileChange = async (filePath) => {
                if (filePath.startsWith(options.pagesDir)) {
                    if (options.onRoutesChanged) {
                        await options.onRoutesChanged();
                    }
                }
            };
            server.watcher.on('add', handleFileChange);
            server.watcher.on('unlink', handleFileChange);
        },
        handleHotUpdate(ctx) {
            if (ctx.file.startsWith(options.pagesDir)) {
                // Return modules to hot-reload in the browser
                return ctx.modules;
            }
        },
    };
}
//# sourceMappingURL=vite-plugin.js.map