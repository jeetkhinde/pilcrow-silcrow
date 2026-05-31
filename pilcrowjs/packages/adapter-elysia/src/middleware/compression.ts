import { Elysia } from 'elysia';
// @ts-ignore
import { compression as elysiaCompress } from 'elysia-compress';

export const compression = () => (app: Elysia) => {
  return app.use(elysiaCompress());
};
