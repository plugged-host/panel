// plugged.host: an admin deletes a ticket (docs/features/tickets.md)
import { axiosInstance } from '@/api/axios.ts';

export default async (uuid: string): Promise<void> => {
  await axiosInstance.delete(`/api/admin/tickets/${uuid}`);
};
