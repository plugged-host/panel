import { z } from 'zod';
import { serverDirectoryEntrySchema } from '@/lib/schemas/server/files.ts';

type DirectoryEntry = z.infer<typeof serverDirectoryEntrySchema>;

export const isMountGateway = (file: DirectoryEntry, browsingPrimaryFilesystem: boolean) =>
  browsingPrimaryFilesystem && file.virtual;

export const withoutMountGateways = (files: DirectoryEntry[], browsingPrimaryFilesystem: boolean) =>
  files.filter((file) => !isMountGateway(file, browsingPrimaryFilesystem));

export const hasNonMountGateway = (files: DirectoryEntry[], browsingPrimaryFilesystem: boolean) =>
  files.some((file) => !isMountGateway(file, browsingPrimaryFilesystem));
