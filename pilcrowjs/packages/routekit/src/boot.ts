import React from 'react';
import ReactDOMServer from 'react-dom/server';
import { discoverRoutes } from './discover.js';
import { composeLayoutChain } from './layout-chain.js';
import { extractPageOptions, extractLiveFields } from './page-options.js';
import type { PageRoute, LayoutNode } from './manifest.js';
import type {
  PilcrowRequest,
  PilcrowResponse,
  PilcrowConfig,
  ServerAdapter,
} from '@pilcrowjs/core';

export function buildPageHandler(
  module: any,
  pageMeta: PageRoute,
  layouts: LayoutNode[],
  config: PilcrowConfig
) {
  return async (req: PilcrowRequest, res: PilcrowResponse) => {
    // 1. Execute page load if it exists
    let props: any = {};
    if (typeof module.load === 'function') {
      try {
        props = await module.load(req);
      } catch (err: any) {
        if (err.type === 'Redirect') {
          res.redirect(err.message, err.status);
          return;
        }
        throw err;
      }
    }

    // 2. Extract options and live fields for runtime FSR checks
    const options = extractPageOptions(module);
    const liveFields = extractLiveFields(props);
    pageMeta.liveFields = liveFields;
    pageMeta.promoteAfter = options.promoteAfter;

    // 3. Resolve layout components from the manifest
    const layoutConfigs = [];
    for (const layoutPath of pageMeta.layouts) {
      const layoutNode = layouts.find((l) => l.filePath === layoutPath);
      const layoutPattern = layoutNode ? layoutNode.pattern : '/';
      const layoutModule = await import(layoutPath);
      layoutConfigs.push({
        pattern: layoutPattern,
        component: layoutModule.default,
      });
    }

    // 4. Render React tree
    const pageComponent = module.default;
    const reactTree = composeLayoutChain(pageComponent, layoutConfigs, pageMeta.pattern, props);
    const bodyHtml = ReactDOMServer.renderToString(reactTree);

    const finalHtml = bodyHtml.startsWith('<html') ? '<!DOCTYPE html>' + bodyHtml : bodyHtml;
    res.html(finalHtml);
  };
}

export function buildActionHandler(actions: Record<string, any>) {
  return async (req: PilcrowRequest, res: PilcrowResponse) => {
    let actionName = '';
    for (const key of Object.keys(req.query)) {
      if (key.startsWith('/')) {
        actionName = key.slice(1);
        break;
      }
    }

    if (!actionName || !actions[actionName]) {
      res.status = 404;
      res.json({ error: `Action "${actionName}" not found` });
      return;
    }

    try {
      const result = await actions[actionName](req);
      res.json(result || { success: true });
    } catch (err: any) {
      if (err.type === 'Redirect') {
        res.redirect(err.message, err.status);
        return;
      }
      res.status = err.status || 500;
      res.json({ error: err.message || 'Action failed' });
    }
  };
}

export async function startPilcrow(
  adapter: ServerAdapter,
  config: PilcrowConfig,
  pagesDir: string
) {
  // 1. Discover routes
  const manifest = await discoverRoutes(pagesDir);

  // 2. Apply middleware
  adapter.applyMiddleware({
    csrf: true,
    timeoutMs: 30000,
    compression: true,
  });

  // 3. Register page routes
  for (const page of manifest.pages) {
    const module = await import(page.filePath);
    adapter.registerPage(
      page.pattern,
      page.layouts,
      buildPageHandler(module, page, manifest.layouts, config)
    );
    if (module.actions) {
      adapter.registerAction(page.pattern, buildActionHandler(module.actions));
    }
  }

  // 4. Register FSR SSE hubs (stubs)
  adapter.registerSSE('/__pilcrow/fsr', async (req, res) => {
    res.sse({
      async *[Symbol.asyncIterator]() {
        yield { event: 'ping', data: 'hello' };
      },
    });
  });

  adapter.registerSSE('/__pilcrow/live/*', async (req, res) => {
    res.sse({
      async *[Symbol.asyncIterator]() {
        yield { event: 'ping', data: 'hello' };
      },
    });
  });

  return manifest;
}
