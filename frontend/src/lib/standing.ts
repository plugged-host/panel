// plugged.host: an account's standing levels (shared/src/standing.rs, docs/features/standing.md)

const LEVEL_KEYS = new Map([
  ['good', 'good'],
  ['warning', 'warning'],
  ['limited', 'limited'],
  ['very_limited', 'veryLimited'],
  ['suspended', 'suspended'],
]);

/** The translation key of a standing level, `null` for a level this panel does not know. */
export function standingLevelKey(level: string): string | null {
  const key = LEVEL_KEYS.get(level);
  return key ? `common.enum.accountStanding.${key}` : null;
}
