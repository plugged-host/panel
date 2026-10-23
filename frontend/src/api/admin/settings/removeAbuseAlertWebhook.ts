// plugged.host: `null` removes the stored abuse alert webhook (an empty field keeps it).
import { axiosInstance } from '@/api/axios.ts';

export default async (): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', { abuse: { alert_webhook: null } });
};
