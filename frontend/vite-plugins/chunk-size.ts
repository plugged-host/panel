import type { Plugin } from 'vite';

interface ChunkSizeWarningsOptions {
  limitKb: number;
  ignoreModules?: RegExp;
  ignoreAssets?: RegExp;
}

export function chunkSizeWarnings({ limitKb, ignoreModules, ignoreAssets }: ChunkSizeWarningsOptions): Plugin {
  const limitBytes = limitKb * 1000;
  let oversized: string[] = [];

  return {
    name: 'chunk-size-warnings',
    apply: 'build',
    enforce: 'post',

    writeBundle(_options, bundle) {
      oversized = [];

      for (const output of Object.values(bundle)) {
        if (!output.fileName.endsWith('.js')) continue;

        if (output.type === 'chunk') {
          const size = Buffer.byteLength(output.code);
          if (size <= limitBytes) continue;

          const moduleIds = output.moduleIds.filter((id) => !id.startsWith('\0'));
          if (ignoreModules && moduleIds.length > 0 && moduleIds.every((id) => ignoreModules.test(id))) continue;

          oversized.push(`${output.fileName} (${(size / 1000).toFixed(2)} kB)`);
        } else {
          const size = Buffer.byteLength(output.source);
          if (size <= limitBytes) continue;
          if (ignoreAssets?.test(output.fileName)) continue;

          oversized.push(`${output.fileName} (${(size / 1000).toFixed(2)} kB)`);
        }
      }
    },

    closeBundle() {
      if (oversized.length > 0) {
        this.warn(`Chunks larger than ${limitKb} kB after minification:\n  ${oversized.join('\n  ')}`);
      }
    },
  };
}
