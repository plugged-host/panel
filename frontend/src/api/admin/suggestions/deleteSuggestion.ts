// plugged.host: deleting an idea (docs/features/suggestions.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/admin/suggestions/${uuid}`);
};
