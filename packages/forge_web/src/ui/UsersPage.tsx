import { ActionIcon, Avatar, Badge, Button, Card, Chip, Group, PasswordInput, Stack, Switch, Table, Text, TextInput, Title, Tooltip } from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { modals } from '@mantine/modals';
import { IconArrowLeft, IconCheck, IconSearch, IconTrash, IconUserPlus } from '@tabler/icons-react';
import { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router';

import { ApiError, type ForgeUser, type Json, userName } from '../api/client';
import { useForge } from '../context';
import { useAsync } from '../hooks';
import { paths } from '../paths';
import { ErrorView, Loading, PageHeader, Pager, useNotify } from './common';

const PER_PAGE = 25;

/** Comptes utilisateurs (administrateurs). */
export function UsersPage() {
  const forge = useForge();
  const { strings } = forge;
  const navigate = useNavigate();
  const [page, setPage] = useState(1);
  const [search, setSearch] = useState('');
  const [debounced] = useDebouncedValue(search, 300);
  const listing = useAsync(
    () =>
      forge.client.get('/api/users', {
        page: String(page),
        per_page: String(PER_PAGE),
        ...(debounced.trim() ? { q: debounced.trim() } : {}),
      }) as Promise<Json>,
    [page, debounced],
  );
  return (
    <Stack maw={1000} mx="auto">
      <PageHeader
        title={strings.users}
        actions={
          <Button component={Link} to={paths.newUser} leftSection={<IconUserPlus size={18} />} variant="gradient">
            {strings.newUser}
          </Button>
        }
      />
      <TextInput
        placeholder={strings.search}
        aria-label={strings.search}
        leftSection={<IconSearch size={16} />}
        value={search}
        onChange={(e) => {
          setSearch(e.currentTarget.value);
          setPage(1);
        }}
      />
      {listing.error ? (
        <ErrorView error={listing.error} onRetry={listing.reload} />
      ) : !listing.data ? (
        <Loading />
      ) : (
        <Card p={0}>
          <Table highlightOnHover verticalSpacing="sm" horizontalSpacing="md">
            <Table.Tbody>
              {(listing.data.data as ForgeUser[]).map((user) => (
                <Table.Tr key={user.id} onClick={() => navigate(paths.user(user.id))} style={{ cursor: 'pointer' }}>
                  <Table.Td>
                    <Group gap="sm" wrap="nowrap">
                      <Avatar color={user.active ? 'indigo' : 'gray'} radius="xl">
                        {userName(user).slice(0, 2).toUpperCase()}
                      </Avatar>
                      <div>
                        <Text fw={600}>{userName(user)}</Text>
                        <Text size="sm" c="dimmed">
                          {user.email}
                        </Text>
                      </div>
                    </Group>
                  </Table.Td>
                  <Table.Td>
                    <Group gap={4}>
                      {user.roles.map((r) => (
                        <Badge key={r} variant="light">
                          {r}
                        </Badge>
                      ))}
                    </Group>
                  </Table.Td>
                  <Table.Td>{!user.active && <Badge color="gray">{strings.active} ✕</Badge>}</Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
          {(() => {
            const total = listing.data.total as number;
            const count = (listing.data.data as unknown[]).length;
            const first = (page - 1) * PER_PAGE + 1;
            return (
              <Pager
                page={page}
                pages={Math.max(1, Math.ceil(total / PER_PAGE))}
                first={count ? first : 0}
                last={first + count - 1}
                total={total}
                onPage={setPage}
              />
            );
          })()}
        </Card>
      )}
    </Stack>
  );
}

/** Création (`id` absent) ou modification d'un compte. */
export function UserFormPage({ id }: { id?: number }) {
  const forge = useForge();
  const { strings } = forge;
  const navigate = useNavigate();
  const notify = useNotify();
  const creating = id === undefined;
  const [user, setUser] = useState<ForgeUser | null>(null);
  const [email, setEmail] = useState('');
  const [name, setName] = useState('');
  const [password, setPassword] = useState('');
  const [roles, setRoles] = useState<string[]>([]);
  const [active, setActive] = useState(true);
  const [errors, setErrors] = useState<Record<string, string[]>>({});
  const [saving, setSaving] = useState(false);
  const [loadError, setLoadError] = useState<unknown>(null);

  useEffect(() => {
    if (creating) return;
    forge.client
      .get(`/api/users/${id}`)
      .then((json) => {
        const loaded = json as ForgeUser;
        setUser(loaded);
        setEmail(loaded.email);
        setName(loaded.display_name ?? '');
        setRoles(loaded.roles);
        setActive(loaded.active);
      })
      .catch(setLoadError);
  }, [forge.client, creating, id]);

  const save = async () => {
    const body = {
      email: email.trim(),
      display_name: name.trim() || null,
      roles,
      active,
      ...(password ? { password } : {}),
    };
    setSaving(true);
    try {
      if (creating) await forge.client.post('/api/users', body);
      else await forge.client.patch(`/api/users/${id}`, body);
      // Son propre compte : nom et rôles affichés à jour.
      if (id === forge.user?.id) await forge.client.reloadUser();
      forge.notifyChange();
      navigate(paths.users);
    } catch (e) {
      if (e instanceof ApiError && Object.keys(e.fields).length) setErrors(e.fields);
      else notify.error(e);
    } finally {
      setSaving(false);
    }
  };

  const remove = () =>
    modals.openConfirmModal({
      title: strings.delete,
      children: <Text size="sm">{strings.confirmDelete(user ? userName(user) : '')}</Text>,
      labels: { confirm: strings.delete, cancel: strings.cancel },
      confirmProps: { color: 'red' },
      onConfirm: async () => {
        try {
          await forge.client.delete(`/api/users/${id}`);
          forge.notifyChange();
          navigate(paths.users);
        } catch (e) {
          notify.error(e);
        }
      },
    });

  if (loadError) return <ErrorView error={loadError} />;
  if (!creating && !user) return <Loading />;
  const error = (field: string) => errors[field]?.join(' ');
  return (
    <Stack maw={640} mx="auto">
      <Group justify="space-between">
        <Group gap="sm">
          <ActionIcon variant="subtle" size="lg" onClick={() => navigate(paths.users)} aria-label="←">
            <IconArrowLeft size={20} />
          </ActionIcon>
          <Title order={2}>{creating ? strings.newUser : userName(user!)}</Title>
        </Group>
        <Group gap="xs">
          {!creating && user!.id !== forge.user?.id && (
            <Tooltip label={strings.delete}>
              <ActionIcon variant="light" color="red" size="lg" onClick={remove} aria-label={strings.delete}>
                <IconTrash size={18} />
              </ActionIcon>
            </Tooltip>
          )}
          <Button onClick={() => void save()} loading={saving} leftSection={<IconCheck size={18} />} variant="gradient">
            {strings.save}
          </Button>
        </Group>
      </Group>
      <Card p="lg">
        <Stack>
          <TextInput label={strings.email} withAsterisk type="email" value={email} error={error('email')} onChange={(e) => setEmail(e.currentTarget.value)} />
          <TextInput label={strings.displayName} value={name} error={error('display_name')} onChange={(e) => setName(e.currentTarget.value)} />
          <PasswordInput
            label={creating ? strings.password : strings.newPassword}
            withAsterisk={creating}
            description={creating ? undefined : strings.keepPasswordHint}
            value={password}
            error={error('password')}
            onChange={(e) => setPassword(e.currentTarget.value)}
          />
          <div>
            <Text size="sm" fw={500} mb={6}>
              {strings.roles}
            </Text>
            <Chip.Group multiple value={roles} onChange={setRoles}>
              <Group gap="xs">
                {forge.schema.roles.map((r) => (
                  <Chip key={r} value={r}>
                    {r}
                  </Chip>
                ))}
              </Group>
            </Chip.Group>
            {error('roles') && (
              <Text size="xs" c="red" mt={4}>
                {error('roles')}
              </Text>
            )}
          </div>
          <Switch label={strings.active} checked={active} onChange={(e) => setActive(e.currentTarget.checked)} />
        </Stack>
      </Card>
    </Stack>
  );
}
