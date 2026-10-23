// plugged.host: limbo helpers (docs/features/limbo.md)
import { z } from 'zod';
import { serverLimboStateSchema } from '@/lib/schemas/server/limbo.ts';

/** A stopped server is an ordinary server; every other limbo state is off its node or on the way. */
export function isInLimbo(state: z.infer<typeof serverLimboStateSchema> | null | undefined): boolean {
  return !!state && state !== 'stopped';
}
