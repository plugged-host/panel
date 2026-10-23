// plugged.host: Admin › Status
import { axiosInstance } from '@/api/axios.ts';

export default async (overrideUuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/admin/status/overrides/${overrideUuid}`);
};
