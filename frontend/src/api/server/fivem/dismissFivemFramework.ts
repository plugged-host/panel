// plugged.host: forget a finished framework install (docs/features/fivem.md#frameworks)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/client/servers/${uuid}/fivem/frameworks/install`);
};
