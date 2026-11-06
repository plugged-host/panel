// plugged.host: an admin deletes a server's custom domain (docs/features/subdomains.md#custom-domains)
import { axiosInstance } from '@/api/axios.ts';

export default async (serverUuid: string, customUuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/admin/servers/${serverUuid}/subdomains/custom/${customUuid}`);
};
