// @ts-ignore
import { compression as elysiaCompress } from 'elysia-compress';
export const compression = () => (app) => {
    return app.use(elysiaCompress());
};
//# sourceMappingURL=compression.js.map