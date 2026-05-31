export const layoutIntercept = () => (app) => {
    return app.derive(({ request }) => {
        const isEnhanced = request.headers.get('silcrow-target') !== null;
        const layoutsPresent = (request.headers.get('x-ps-present') || '')
            .split(',')
            .map((s) => s.trim())
            .filter(Boolean);
        return {
            isEnhanced,
            layoutsPresent,
        };
    });
};
//# sourceMappingURL=layout-intercept.js.map