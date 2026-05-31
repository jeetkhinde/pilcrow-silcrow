#!/usr/bin/env node
import { defineCommand, runMain } from 'citty';
import consola from 'consola';
import * as fs from 'fs/promises';
import * as path from 'path';
import {
  packageJson,
  tsconfigJson,
  pilcrowConfig,
  layout,
  indexPage,
  healthApi,
  drizzleConfig,
  schema,
  migrationSql,
  mainRunner,
} from './templates.js';

const main = defineCommand({
  meta: {
    name: 'create-pilcrow',
    version: '0.1.0',
    description: 'Scaffolding utility for Pilcrow.js apps',
  },
  args: {
    dir: {
      type: 'positional',
      description: 'Directory to scaffold the app into',
      required: false,
    },
  },
  async run({ args }) {
    const targetDirName = args.dir || 'pilcrow-app';
    const targetDir = path.resolve(process.cwd(), targetDirName);

    consola.info(`Scaffolding new Pilcrow.js app in ${targetDir}...`);

    try {
      await fs.mkdir(targetDir, { recursive: true });
      await fs.mkdir(path.join(targetDir, 'pages'), { recursive: true });
      await fs.mkdir(path.join(targetDir, 'api'), { recursive: true });
      await fs.mkdir(path.join(targetDir, 'db'), { recursive: true });
      await fs.mkdir(path.join(targetDir, 'drizzle'), { recursive: true });
      await fs.mkdir(path.join(targetDir, 'src'), { recursive: true });

      // Write files
      await fs.writeFile(path.join(targetDir, 'package.json'), packageJson(targetDirName));
      await fs.writeFile(path.join(targetDir, 'tsconfig.json'), tsconfigJson);
      await fs.writeFile(path.join(targetDir, 'pilcrow.config.ts'), pilcrowConfig);
      await fs.writeFile(path.join(targetDir, 'pages/_layout.tsx'), layout);
      await fs.writeFile(path.join(targetDir, 'pages/index.tsx'), indexPage);
      await fs.writeFile(path.join(targetDir, 'api/health.ts'), healthApi);
      await fs.writeFile(path.join(targetDir, 'drizzle.config.ts'), drizzleConfig);
      await fs.writeFile(path.join(targetDir, 'db/schema.ts'), schema);
      await fs.writeFile(path.join(targetDir, 'drizzle/0000_init.sql'), migrationSql);
      await fs.writeFile(path.join(targetDir, 'src/main.ts'), mainRunner);

      consola.success(`Successfully scaffolded Pilcrow.js app inside ${targetDirName}!`);
      consola.box({
        title: 'Next steps:',
        style: {
          padding: 1,
          borderColor: 'green',
          borderStyle: 'double',
        },
        text: `cd ${targetDirName}\npnpm install\npnpm run dev`,
      });
    } catch (err: any) {
      consola.error(`Failed to scaffold app: ${err.message}`);
      process.exit(1);
    }
  },
});

runMain(main);
