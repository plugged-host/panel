import { axiosInstance } from '@/api/axios.ts';

export default async (): Promise<string[]> => {
  const { data } = await axiosInstance.get('/api/timezones');
  return data.timezones;
};
