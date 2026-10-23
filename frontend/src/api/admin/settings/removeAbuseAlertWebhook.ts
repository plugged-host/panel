// plugged.host: Admin › Settings › Abuse: `null` removes the stored webhook (an empty field keeps it).
import { axiosInstance } from '@/api/axios.ts';

export default async (): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', { abuse: { alert_webhook: null } });
};
