// plugged.host: the Minecraft versions a software choice offers, newest first
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string, variant: string | null): Promise<string[]> => {
  const { data } = await axiosInstance.get(`/api/client/servers/${uuid}/settings/software/versions`, {
    params: variant ? { variant } : {},
  });
  return data.versions;
};
