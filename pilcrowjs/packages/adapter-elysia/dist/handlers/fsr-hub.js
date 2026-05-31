export async function handleFsrHub(req, res) {
    res.sse({
        async *[Symbol.asyncIterator]() {
            yield { event: 'ping', data: 'hello' };
        },
    });
}
//# sourceMappingURL=fsr-hub.js.map