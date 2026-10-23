// plugged.host: Admin › Abuse
import { axiosInstance } from '@/api/axios.ts';

export default async (findingUuid: string, status: 'dismissed' | 'resolved'): Promise<void> => {
  await axiosInstance.patch(`/api/admin/abuse/findings/${findingUuid}`, { status });
};
