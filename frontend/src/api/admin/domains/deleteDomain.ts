// plugged.host: delete a domain with its subdomains (docs/features/subdomains.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/admin/domains/${uuid}`);
};
