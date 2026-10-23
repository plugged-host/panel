import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ObjectSet } from '@/lib/objectSet.ts';
import { StorageAsset } from '@/lib/schemas/admin/assets.ts';
import { selectionRange } from '@/plugins/selection/selectionRange.ts';

export type AssetSet = ObjectSet<StorageAsset, 'name'>;

export function emptyAssetSet(entries?: StorageAsset[]): AssetSet {
  return new ObjectSet<StorageAsset, 'name'>('name', entries);
}

export function useAssetSelection(assets: StorageAsset[] | undefined) {
  const [selected, setSelected] = useState<AssetSet>(() => emptyAssetSet());

  const selectableAssets = useMemo(() => (assets ?? []).filter((asset) => !asset.isDirectory), [assets]);

  const clear = useCallback(() => setSelected(emptyAssetSet()), []);

  const add = useCallback((asset: StorageAsset) => {
    if (asset.isDirectory) return;
    setSelected((prev) => prev.clone().add(asset));
  }, []);

  const remove = useCallback((asset: StorageAsset) => {
    setSelected((prev) => {
      const next = prev.clone();
      next.delete(asset);
      return next;
    });
  }, []);

  const selectableAssetsRef = useRef(selectableAssets);
  const anchorRef = useRef<string | null>(null);

  useEffect(() => {
    selectableAssetsRef.current = selectableAssets;
  });

  const toggle = useCallback((asset: StorageAsset, range = false) => {
    if (asset.isDirectory) return;
    const affected = range
      ? selectionRange(selectableAssetsRef.current, anchorRef.current, asset, (entry) => entry.name)
      : [asset];
    anchorRef.current = asset.name;

    setSelected((prev) => {
      const next = prev.clone();
      const selecting = !next.has(asset);
      for (const entry of affected) {
        if (selecting) {
          next.add(entry);
        } else {
          next.delete(entry);
        }
      }
      return next;
    });
  }, []);

  const replace = useCallback((next: StorageAsset[]) => {
    setSelected(emptyAssetSet(next.filter((asset) => !asset.isDirectory)));
  }, []);

  const selectAll = useCallback(() => setSelected(emptyAssetSet(selectableAssets)), [selectableAssets]);

  return { selected, selectableAssets, clear, add, remove, toggle, replace, selectAll };
}
