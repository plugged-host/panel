// plugged.host: a new devbox CA and edge certificate (docs/features/devboxes.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (): Promise<void> => {
  await axiosInstance.put('/api/admin/settings', {
    devboxes: { create_certificates: true },
  });
};
