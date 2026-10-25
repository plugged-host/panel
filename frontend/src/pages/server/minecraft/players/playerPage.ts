// plugged.host: what the sections of one player's page share (docs/features/minecraft.md#player-details)
import { z } from 'zod';
import { minecraftPlayerDataSchema, minecraftPlayerDetailsSchema } from '@/lib/schemas/server/minecraft.ts';

export type PlayerDetails = z.infer<typeof minecraftPlayerDetailsSchema>;
export type PlayerData = z.infer<typeof minecraftPlayerDataSchema>;

export const playerBase = 'pages.server.minecraft.players.details';

export interface PlayerPage {
  details: PlayerDetails;
  /** The name, or the UUID while the server does not know the name. */
  name: string;
  running: boolean;
  stopped: boolean;
  /** Whether the player is on the server; `null` when it cannot be told. */
  online: boolean | null;
  /** Why the console's actions cannot be used now; `null` when they can. */
  liveBlocked: string | null;
  /** Why a change (game mode, teleport) cannot be made now: live while running, in the file while stopped. */
  changeBlocked: string | null;
  canEdit: boolean;
  canDelete: boolean;
  /** Shows a console change right away: the files only have it from the next save. */
  patch: (change: (data: PlayerData) => PlayerData) => void;
  /** Reads the files again, after a change to them. */
  reload: () => void;
  /** Says a change is done; one that went into the file applies when the player next joins. */
  done: (message: string, live: boolean) => void;
}
