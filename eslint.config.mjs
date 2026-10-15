import eslintConfig from '@bob-park/eslint-config-bobpark';

import { defineConfig } from 'eslint/config';

export default defineConfig([
  {
    extends: [eslintConfig],
  },
  {
    // motion.test.ts runs floatCard under `node --test`, which cannot resolve the `@/` alias,
    // so these two files reach utils/motion.ts through a relative path.
    files: ['src/app/panel/_components/motion.ts', 'src/app/panel/_components/cardImage.ts'],
    // Same rule as the shared config, minus the `../*` pattern.
    rules: {
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              name: 'react',
              importNames: ['default'],
              message: "import React from 'react' makes bundle size larger.",
            },
          ],
        },
      ],
    },
  },
]);
