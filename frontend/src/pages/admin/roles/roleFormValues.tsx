import { z } from 'zod';
import { type FieldDef } from '@/elements/form-engine/index.ts';
import PermissionSelector from '@/elements/PermissionSelector.tsx';
import { adminRoleUpdateSchema } from '@/lib/schemas/admin/roles.ts';
import { apiPermissionsSchema, permissionMapSchema } from '@/lib/schemas/generic.ts';
import { roleSchema } from '@/lib/schemas/user.ts';
import { useTranslations } from '@/providers/TranslationProvider.tsx';

type RoleFormValues = z.infer<typeof adminRoleUpdateSchema>;
type PermissionMapType = 'serverPermissions' | 'adminPermissions';

export const roleEmptyFormValues: RoleFormValues = {
  name: '',
  description: null,
  requireTwoFactor: false,
  adminPermissions: [],
  serverPermissions: [],
};

export const roleToFormValues = (role: z.infer<typeof roleSchema>): Partial<RoleFormValues> => ({
  name: role.name,
  description: role.description,
  requireTwoFactor: role.requireTwoFactor,
  adminPermissions: role.adminPermissions,
  serverPermissions: role.serverPermissions,
});

// plugged.host: what the staff preset leaves out: the panel's settings (economy, neo, ads, …) and what
// works like them, node tokens, commands past the abuse rules and changing roles (docs/features/management.md)
const STAFF_EXCLUDED_GROUPS = ['settings', 'email-templates', 'extensions', 'oauth-providers'];
const STAFF_EXCLUDED_PERMISSIONS = [
  'roles.create',
  'roles.update',
  'roles.delete',
  'nodes.read-token',
  'nodes.reset-token',
  'nodes.upgrade',
  'servers.startup',
];

export const SETTINGS_PERMISSIONS = ['settings.update', 'email-templates.update', 'extensions.manage'];

// plugged.host: every admin permission but the excluded ones, and every server permission
export function staffPreset(availablePermissions: z.infer<typeof apiPermissionsSchema>) {
  const all = (map: z.infer<typeof permissionMapSchema>) =>
    Object.entries(map).flatMap(([group, { permissions }]) =>
      Object.keys(permissions).map((permission) => `${group}.${permission}`),
    );

  return {
    adminPermissions: all(availablePermissions.adminPermissions).filter(
      (permission) =>
        !STAFF_EXCLUDED_GROUPS.includes(permission.split('.')[0]) && !STAFF_EXCLUDED_PERMISSIONS.includes(permission),
    ),
    serverPermissions: all(availablePermissions.serverPermissions),
  };
}

export function useRoleFormFields(
  availablePermissions: z.infer<typeof apiPermissionsSchema>,
): FieldDef<RoleFormValues>[] {
  const { t } = useTranslations();

  const permissionField = (name: PermissionMapType, label: string): FieldDef<RoleFormValues> => ({
    type: 'custom',
    name,
    colSpan: 'full',
    render: (f) => (
      <PermissionSelector
        label={label}
        permissionsMapType={name}
        permissions={availablePermissions[name]}
        selectedPermissions={f.getValues()[name]}
        setSelectedPermissions={(selected) => f.setFieldValue(name, selected)}
      />
    ),
  });

  return [
    { type: 'text', name: 'name', label: t('common.form.name', {}), required: true },
    { type: 'textarea', name: 'description', label: t('common.form.description', {}), rows: 3 },
    {
      type: 'switch',
      name: 'requireTwoFactor',
      label: t('pages.admin.roles.tabs.general.page.form.requireTwoFactor', {}),
      description: t('pages.admin.roles.tabs.general.page.form.requireTwoFactorDescription', {}),
    },
    permissionField('serverPermissions', t('pages.admin.roles.tabs.general.page.form.serverPermissions', {})),
    permissionField('adminPermissions', t('pages.admin.roles.tabs.general.page.form.adminPermissions', {})),
  ];
}
