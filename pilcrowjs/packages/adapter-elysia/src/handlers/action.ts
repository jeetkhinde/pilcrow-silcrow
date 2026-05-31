import type { PilcrowRequest, PilcrowResponse } from '@pilcrowjs/core';

export async function handleAction(req: PilcrowRequest, res: PilcrowResponse) {
  res.json({ success: true });
}
