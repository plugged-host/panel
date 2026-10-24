// plugged.host: back to a devbox in the browser after the panel's sign-in
import { axiosInstance } from '@/api/axios.ts';

export default async (host: string): Promise<{ url: string | null; server: string | null }> => {
  const { data } = await axiosInstance.post('/api/client/account/devboxes/open', { host });
  return { url: data.url ?? null, server: data.server ?? null };
};
