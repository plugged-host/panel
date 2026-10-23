export function selectionRange<T>(
  items: readonly T[],
  anchor: string | null,
  target: T,
  identify: (item: T) => string,
): T[] {
  const targetIndex = items.findIndex((item) => identify(item) === identify(target));
  const anchorIndex = anchor === null ? -1 : items.findIndex((item) => identify(item) === anchor);
  if (targetIndex === -1 || anchorIndex === -1) return [target];

  return items.slice(Math.min(anchorIndex, targetIndex), Math.max(anchorIndex, targetIndex) + 1);
}
