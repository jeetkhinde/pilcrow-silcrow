export const timeout = (timeoutMs = 30000) => (app) => {
    return app
        .derive(() => {
        const controller = new AbortController();
        const id = setTimeout(() => {
            controller.abort(new Error('Timeout'));
        }, timeoutMs);
        return {
            abortController: controller,
            timeoutId: id,
        };
    })
        .onAfterResponse(({ timeoutId }) => {
        clearTimeout(timeoutId);
    })
        .onError(({ error, set }) => {
        const err = error;
        if (err.message === 'Timeout' || err.name === 'AbortError') {
            set.status = 408;
            return 'Request Timeout';
        }
    });
};
export async function withTimeout(promise, timeoutMs) {
    let timer;
    const timeoutPromise = new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error('Timeout')), timeoutMs);
    });
    try {
        return await Promise.race([promise, timeoutPromise]);
    }
    finally {
        clearTimeout(timer);
    }
}
//# sourceMappingURL=timeout.js.map