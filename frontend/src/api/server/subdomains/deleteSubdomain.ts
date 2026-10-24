// plugged.host: delete a subdomain; its tokens go back to the owner (docs/features/subdomains.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, subdomainUuid: string): Promise<number> => {
  const { data } = await axiosInstance.delete(`/api/client/servers/${uuid}/subdomains/${subdomainUuid}`);
  return data.refunded as number;
};
