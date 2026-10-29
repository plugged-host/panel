// plugged.host: buys (or renews) an upgrade with the price the user saw; 409 when it moved
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverUpgradeKindSchema, serverUpgradesSchema } from '@/lib/schemas/server/upgrades.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export interface ServerUpgradePurchase {
  kind: z.infer<typeof serverUpgradeKindSchema>;
  // the extra slot to renew; a new one without
  renew?: string | null;
  tokens: number;
  color?: string | null;
}

export default async (uuid: string, purchase: ServerUpgradePurchase): Promise<z.infer<typeof serverUpgradesSchema>> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/upgrades`, {
    kind: purchase.kind,
    renew: purchase.renew ?? null,
    tokens: purchase.tokens,
    color: purchase.color ?? null,
  });
  return parseFromApi(serverUpgradesSchema, data.upgrades);
};
