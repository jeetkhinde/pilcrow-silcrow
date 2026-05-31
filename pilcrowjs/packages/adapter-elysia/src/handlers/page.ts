import type { PilcrowRequest, PilcrowResponse } from '@pilcrowjs/core';

export async function handlePage(req: PilcrowRequest, res: PilcrowResponse) {
  res.html('<h1>Pilcrow Page</h1>');
}
