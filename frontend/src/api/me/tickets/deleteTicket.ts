// plugged.host: deleting a ticket of the account (docs/features/tickets.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/client/account/tickets/${uuid}`);
};
