import { axiosInstance } from '@/api/axios.ts';

export default async (nodeUuid: string): Promise<{ serversDeleted: number }> => {
  const { data } = await axiosInstance.post(`/api/admin/nodes/${nodeUuid}/force-nuke`);
  return { serversDeleted: data.serversDeleted };
};
