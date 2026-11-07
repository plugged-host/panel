// plugged.host: deleting an idea the account posted (docs/features/suggestions.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/client/account/suggestions/${uuid}`);
};
