// plugged.host: a git merge driver that merges translation files key by key (docs/upstream.md).
//
//   git config merge.translations.name 'translation files, key by key'
//   git config merge.translations.driver 'node frontend/merge-translations.mjs %O %A %B %P'
//
// .gitattributes sends frontend/public/translations/*.json and shared/mails/variables/*.json here.
// A key changed on one side only takes that side's value (a removal included); a key changed on
// both sides to different values keeps ours and is listed, so the merge never stops on a line
// conflict. Keys come in their order, with ours that they lack after the key they follow in ours.
import fs from 'node:fs';

const [basePath, oursPath, theirsPath, name = oursPath] = process.argv.slice(2);

const read = (path) => {
  try {
    return JSON.parse(fs.readFileSync(path, 'utf8'));
  } catch {
    return undefined;
  }
};

const isObject = (value) => typeof value === 'object' && value !== null && !Array.isArray(value);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const kept = [];

const merge = (base, ours, theirs, path) => {
  if (isObject(ours) && isObject(theirs)) {
    const baseObject = isObject(base) ? base : {};
    const keys = Object.keys(theirs);
    const oursKeys = Object.keys(ours);
    oursKeys.forEach((key, index) => {
      if (keys.includes(key)) return;
      const after = oursKeys.slice(0, index).reverse().find((previous) => keys.includes(previous));
      keys.splice(after === undefined ? 0 : keys.indexOf(after) + 1, 0, key);
    });

    const merged = {};
    for (const key of keys) {
      const value = merge(baseObject[key], ours[key], theirs[key], path ? `${path}.${key}` : key);
      if (value !== undefined) merged[key] = value;
    }
    return merged;
  }

  if (same(ours, theirs)) return ours;
  if (same(ours, base)) return theirs;
  if (same(theirs, base)) return ours;

  kept.push(path);
  return ours;
};

const base = read(basePath);
const ours = read(oursPath);
const theirs = read(theirsPath);

if (!isObject(ours) || !isObject(theirs)) {
  console.error(`${name}: not a JSON object on both sides, resolve it by hand`);
  process.exit(1);
}

fs.writeFileSync(oursPath, `${JSON.stringify(merge(base, ours, theirs, ''), null, 2)}\n`);

for (const path of kept) {
  console.error(`${name}: ${path} changed on both sides, kept ours`);
}
