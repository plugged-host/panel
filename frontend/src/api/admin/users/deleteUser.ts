import { axiosInstance } from '@/api/axios.ts';

// plugged.host: `deleteServers` deletes the user's servers first, `force` even on unreachable nodes
export default async (userUuid: string, options: { deleteServers?: boolean; force?: boolean } = {}): Promise<void> => {
  await axiosInstance.delete(`/api/admin/users/${userUuid}`, {
    params: options.deleteServers ? { delete_servers: true, force: options.force ?? false } : undefined,
  });
};
