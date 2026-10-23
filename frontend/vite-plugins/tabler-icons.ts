import path from 'node:path';
import type { Plugin } from 'vite';

// plugged.host: every `@fortawesome/react-fontawesome` import in the panel's own code resolves to
// src/lib/icons/FontAwesomeIcon.tsx, which draws Tabler's outline icons (the Pelican panel's) and
// hands the rest to the real component. The shim itself and node_modules get the real package.
const shim = path.resolve(import.meta.dirname, '../src/lib/icons/FontAwesomeIcon.tsx');

export function tablerIcons(): Plugin {
  return {
    name: 'tabler-icons',
    enforce: 'pre',

    resolveId(id, importer) {
      if (id !== '@fortawesome/react-fontawesome' || !importer) return null;

      const from = path.resolve(importer.split('?')[0]);
      if (from === shim || from.includes(`${path.sep}node_modules${path.sep}`)) return null;

      return shim;
    },
  };
}
