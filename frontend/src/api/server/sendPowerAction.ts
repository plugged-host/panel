import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { serverPowerAction } from '@/lib/schemas/server/server.ts';

// plugged.host: a start gate that holds a start answers 202 with its name and page
// (shared/src/start_gates.rs)
export interface PowerActionResult {
  gate?: string;
  url?: string;
}

export default async (uuid: string, action: z.infer<typeof serverPowerAction>): Promise<PowerActionResult> => {
  const { data } = await axiosInstance.post(`/api/client/servers/${uuid}/power`, {
    action,
  });

  return data ?? {};
};
