// plugged.host: claim a first-week task (docs/features/economy.md#getting-started)
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { starterTaskSchema } from '@/lib/schemas/user/economy.ts';

export default async (task: z.infer<typeof starterTaskSchema>): Promise<number> => {
  const { data } = await axiosInstance.post('/api/client/account/tokens/tasks', { task });
  return Number(data.claim?.amount ?? 0);
};
