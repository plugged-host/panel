// plugged.host: Admin › Sponsored Banners
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/admin/ad-banners/${uuid}`);
};
