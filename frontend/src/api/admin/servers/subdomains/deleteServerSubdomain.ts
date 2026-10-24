// plugged.host: delete a server's subdomain as an admin; its tokens go back to the owner (docs/features/subdomains.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (serverUuid: string, subdomainUuid: string): Promise<number> => {
  const { data } = await axiosInstance.delete(`/api/admin/servers/${serverUuid}/subdomains/${subdomainUuid}`);
  return data.refunded as number;
};
