// plugged.host: limbo helpers (docs/features/limbo.md)
import { z } from 'zod';
import { serverLimboStateSchema } from '@/lib/schemas/server/limbo.ts';

/** A stopped server is an ordinary server; every other limbo state is off its node or on the way. */
export function isInLimbo(state: z.infer<typeof serverLimboStateSchema> | null | undefined): boolean {
  return !!state && state !== 'stopped';
}

/** Parked: off its node, or in place (`dormant`: suspended on its node); waits for a restore. */
export function isParked(state: z.infer<typeof serverLimboStateSchema> | null | undefined): boolean {
  return state === 'parked' || state === 'dormant';
}
