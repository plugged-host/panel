// plugged.host: a server's own subdomain limit; null follows the settings (docs/features/subdomains.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (serverUuid: string, limit: number | null): Promise<void> => {
  await axiosInstance.put(`/api/admin/servers/${serverUuid}/subdomains`, { limit });
};
