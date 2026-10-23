// plugged.host: Admin › Plans
import { axiosInstance } from '@/api/axios.ts';

export default async (planUuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/admin/plans/${planUuid}`);
};
