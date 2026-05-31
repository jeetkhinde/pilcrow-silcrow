export async function handleLiveHub(req, res) {
    res.sse({
        async *[Symbol.asyncIterator]() {
            yield { event: 'ping', data: 'hello' };
        },
    });
}
//# sourceMappingURL=live-hub.js.map