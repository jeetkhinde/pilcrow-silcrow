import pg from 'pg';
export async function startDbNotificationPipeline(connectionString, store, watcher) {
    const client = new pg.Client({ connectionString });
    await client.connect();
    await client.query('LISTEN pilcrow_invalidate');
    client.on('notification', async (msg) => {
        if (msg.channel === 'pilcrow_invalidate' && msg.payload) {
            try {
                const payload = JSON.parse(msg.payload);
                const { depKey, id } = payload;
                if (depKey) {
                    // Notify collection key
                    watcher.notifyChange(depKey);
                    // Notify dynamic row-specific key (e.g. tickets:42)
                    if (id !== undefined && id !== null) {
                        watcher.notifyChange(`${depKey}:${id}`);
                    }
                }
            }
            catch (err) {
                console.error('FSR DB listener: failed to parse notification payload:', err.message);
            }
        }
    });
    return client;
}
//# sourceMappingURL=db-notify.js.map