// plugged.host: a download link for a copy on the account's Backups page
import { axiosInstance } from '@/api/axios.ts';

export default async (backupUuid: string): Promise<{ url: string }> => {
  const { data } = await axiosInstance.get(`/api/client/account/backups/${backupUuid}/download`);
  return data;
};
