// plugged.host: Admin › Status
import { axiosInstance } from '@/api/axios.ts';

export default async (incidentUuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/admin/status/incidents/${incidentUuid}`);
};
