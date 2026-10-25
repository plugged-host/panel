// plugged.host: a git merge driver that merges translation files key by key (docs/upstream.md).
//
//   git config merge.translations.name 'translation files, key by key'
//   git config merge.translations.driver 'node frontend/merge-translations.mjs %O %A %B %P'
//
// .gitattributes sends frontend/public/translations/*.json and shared/mails/variables/*.json here.
// A key changed on one side only takes that side's value (a removal included); a key changed on
// both sides to different values keeps ours and is listed, so the merge never stops on a line
// conflict. Keys come in their order, with ours that they lack after the key they follow in ours.
// No dependencies, so it runs in a bare clone. Tests: `node --test merge-translations.test.mjs`.
import fs from 'node:fs';
import { pathToFileURL } from 'node:url';

const isObject = (value) => typeof value === 'object' && value !== null && !Array.isArray(value);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

/**
 * Merges `theirs` into `ours` against their common `base` (undefined for a file new on both sides).
 * Returns the merged object and the paths of the keys both sides changed differently (ours kept).
 */
export function mergeTranslations(base, ours, theirs) {
  const kept = [];

  const merge = (baseValue, oursValue, theirsValue, path) => {
    if (isObject(oursValue) && isObject(theirsValue)) {
      const baseObject = isObject(baseValue) ? baseValue : {};
      const keys = Object.keys(theirsValue);
      const oursKeys = Object.keys(oursValue);
      oursKeys.forEach((key, index) => {
        if (keys.includes(key)) return;
        const after = oursKeys
          .slice(0, index)
          .reverse()
          .find((previous) => keys.includes(previous));
        keys.splice(after === undefined ? 0 : keys.indexOf(after) + 1, 0, key);
      });

      const merged = {};
      for (const key of keys) {
        const value = merge(baseObject[key], oursValue[key], theirsValue[key], path ? `${path}.${key}` : key);
        if (value !== undefined) merged[key] = value;
      }
      return merged;
    }

    if (same(oursValue, theirsValue)) return oursValue;
    if (same(oursValue, baseValue)) return theirsValue;
    if (same(theirsValue, baseValue)) return oursValue;

    kept.push(path);
    return oursValue;
  };

  return { merged: merge(base, ours, theirs, ''), kept };
}

// The file's JSON, or undefined when it is missing, empty or not JSON (git passes an empty base for a new file).
const read = (path) => {
  try {
    return JSON.parse(fs.readFileSync(path, 'utf8'));
  } catch {
    return undefined;
  }
};

/** Git's side, with %O %A %B %P: writes the merge into %A. Returns 1 (a conflict for git) only for unreadable sides. */
export function main([basePath, oursPath, theirsPath, name = oursPath]) {
  const ours = read(oursPath);
  const theirs = read(theirsPath);

  if (!isObject(ours) || !isObject(theirs)) {
    console.error(`${name}: not a JSON object on both sides, resolve it by hand`);
    return 1;
  }

  const { merged, kept } = mergeTranslations(read(basePath), ours, theirs);
  fs.writeFileSync(oursPath, `${JSON.stringify(merged, null, 2)}\n`);

  for (const path of kept) {
    console.error(`${name}: ${path} changed on both sides, kept ours`);
  }

  return 0;
}

const invoked = (() => {
  try {
    return pathToFileURL(fs.realpathSync(process.argv[1])).href;
  } catch {
    return null;
  }
})();

if (import.meta.url === invoked) {
  process.exit(main(process.argv.slice(2)));
}
