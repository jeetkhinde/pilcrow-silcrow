import type { PilcrowRequest, PilcrowResponse } from '@pilcrowjs/core';

export async function handleFsrSnapshot(req: PilcrowRequest, res: PilcrowResponse) {
  res.json({ snapshot: {} });
}
