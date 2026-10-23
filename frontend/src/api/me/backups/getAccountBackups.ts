// plugged.host: the newest copy of each of the account's servers, deleted ones included
import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { userAccountBackupSchema } from '@/lib/schemas/user/backups.ts';
import { parseFromApi } from '@/lib/serialization/api-transform.ts';

export default async (): Promise<z.infer<typeof userAccountBackupSchema>[]> => {
  const { data } = await axiosInstance.get('/api/client/account/backups');
  return parseFromApi(z.array(userAccountBackupSchema), data.backups);
};
