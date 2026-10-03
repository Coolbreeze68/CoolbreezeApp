import {
  ActionIcon,
  AppShell,
  Avatar,
  Burger,
  Group,
  Menu,
  NavLink,
  ScrollArea,
  Text,
  ThemeIcon,
  Tooltip,
  UnstyledButton,
  useComputedColorScheme,
  useMantineColorScheme,
} from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import {
  IconAdjustments,
  IconLayoutDashboard,
  IconLogout,
  IconMoon,
  IconSun,
  IconTable,
  IconUserCircle,
  IconUsers,
} from '@tabler/icons-react';
import type { ReactNode } from 'react';
import { Link, Outlet, useLocation } from 'react-router';

import { userName } from '../api/client';
import { can, isAdmin, rolesOf, useForge } from '../context';
import { paths } from '../paths';

/** Cadre des pages connectées : en-tête, menu latéral (tiroir sur mobile). */
export function Shell() {
  const forge = useForge();
  const { strings, schema, customization, format, user } = forge;
  const [opened, { toggle, close }] = useDisclosure();
  const location = useLocation();
  const { setColorScheme } = useMantineColorScheme();
  const scheme = useComputedColorScheme('light');

  const item = (to: string, label: string, icon: ReactNode, exact = false) => (
    <NavLink
      key={to}
      component={Link}
      to={to}
      label={label}
      leftSection={icon}
      active={exact ? location.pathname === to : location.pathname.startsWith(to)}
      onClick={close}
      variant="light"
      style={{ borderRadius: 'var(--mantine-radius-md)' }}
    />
  );

  return (
    <AppShell
      header={{ height: 60 }}
      navbar={{ width: 264, breakpoint: 'sm', collapsed: { mobile: !opened } }}
      padding="lg"
    >
      <AppShell.Header
        style={{
          background:
            'linear-gradient(120deg, var(--mantine-color-indigo-7), var(--mantine-color-violet-6))',
          color: 'white',
          borderBottom: 0,
        }}
      >
        <Group h="100%" px="md" justify="space-between">
          <Group gap="sm">
            <Burger opened={opened} onClick={toggle} hiddenFrom="sm" size="sm" color="white" />
            <UnstyledButton component={Link} to={paths.home}>
              <Group gap="xs">
                <Avatar color="white" variant="filled" radius="md" size={34}>
                  <Text fw={800} c="indigo.7">
                    {schema.name.slice(0, 1).toUpperCase()}
                  </Text>
                </Avatar>
                <Text fw={700} size="lg" c="white">
                  {schema.name}
                </Text>
              </Group>
            </UnstyledButton>
          </Group>
          <Group gap="xs">
            <Tooltip label={strings.theme}>
              <ActionIcon
                variant="subtle"
                color="white"
                size="lg"
                aria-label={strings.theme}
                onClick={() => setColorScheme(scheme === 'dark' ? 'light' : 'dark')}
              >
                {scheme === 'dark' ? <IconSun size={20} /> : <IconMoon size={20} />}
              </ActionIcon>
            </Tooltip>
            {user && (
              <Menu position="bottom-end" width={220}>
                <Menu.Target>
                  <UnstyledButton aria-label={strings.account}>
                    <Avatar color="white" variant="light" radius="xl">
                      {userName(user).slice(0, 2).toUpperCase()}
                    </Avatar>
                  </UnstyledButton>
                </Menu.Target>
                <Menu.Dropdown>
                  <Menu.Label>{user.email}</Menu.Label>
                  <Menu.Item
                    component={Link}
                    to={paths.account}
                    leftSection={<IconUserCircle size={16} />}
                  >
                    {strings.account}
                  </Menu.Item>
                  <Menu.Item
                    color="red"
                    leftSection={<IconLogout size={16} />}
                    onClick={() => void forge.client.signOut()}
                  >
                    {strings.signOut}
                  </Menu.Item>
                </Menu.Dropdown>
              </Menu>
            )}
          </Group>
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="sm">
        <AppShell.Section grow component={ScrollArea}>
          {item(
            paths.home,
            strings.dashboard,
            <ThemeIcon variant="gradient" size="md" radius="md">
              <IconLayoutDashboard size={16} />
            </ThemeIcon>,
            true,
          )}
          <Text size="xs" fw={700} c="dimmed" tt="uppercase" mt="md" mb={4} px="sm">
            {strings.records}
          </Text>
          {schema.tables
            .filter((t) => can(forge, t, 'read'))
            .map((t) => {
              const Icon = customization.tableIcons?.[t.name] ?? IconTable;
              return item(
                paths.table(t.name),
                format.tableLabel(t),
                <ThemeIcon variant="light" size="md" radius="md">
                  <Icon size={16} />
                </ThemeIcon>,
              );
            })}
          {(customization.pages ?? [])
            .filter((p) => !p.roles || p.roles.some((r) => rolesOf(forge).includes(r)))
            .map((p) =>
              item(
                paths.page(p.path),
                format.label(p.label, p.path),
                <ThemeIcon variant="light" size="md" radius="md" color="grape">
                  <p.icon size={16} />
                </ThemeIcon>,
              ),
            )}
        </AppShell.Section>
        {isAdmin(forge) && (
          <AppShell.Section pt="sm" style={{ borderTop: '1px solid var(--mantine-color-default-border)' }}>
            {(schema.parameters?.length ?? 0) > 0 &&
              item(paths.parameters, strings.parameters, <IconAdjustments size={18} />)}
            {item(paths.users, strings.users, <IconUsers size={18} />)}
          </AppShell.Section>
        )}
      </AppShell.Navbar>

      <AppShell.Main bg={scheme === 'dark' ? 'dark.8' : 'gray.0'}>
        <Outlet />
      </AppShell.Main>
    </AppShell>
  );
}
