import { Card, Group, Paper, SimpleGrid, Stack, Text, ThemeIcon, Title, UnstyledButton } from '@mantine/core';
import { IconCalendarEvent, IconTable } from '@tabler/icons-react';
import { useNavigate } from 'react-router';

import { userName } from '../api/client';
import { can, useForge } from '../context';
import { useAsync } from '../hooks';
import { paths } from '../paths';
import { findColumn, type TableSchema } from '../schema';
import { loadEvents } from './CalendarPanel';
import { Loading } from './common';
import { formatMeasure, GroupBars } from './StatsPanel';

/** Dégradés des indicateurs, dans l'ordre des tables. */
const GRADIENTS = [
  { from: 'indigo', to: 'violet' },
  { from: 'teal', to: 'cyan' },
  { from: 'orange', to: 'pink' },
  { from: 'grape', to: 'pink' },
  { from: 'blue', to: 'cyan' },
  { from: 'lime', to: 'teal' },
];

function TableTile({ table, index }: { table: TableSchema; index: number }) {
  const forge = useForge();
  const navigate = useNavigate();
  const Icon = forge.customization.tableIcons?.[table.name] ?? IconTable;
  const count = useAsync(() => forge.client.table(table.name).list({ perPage: 1 }), [table.name]);
  const gradient = GRADIENTS[index % GRADIENTS.length];
  return (
    <UnstyledButton onClick={() => navigate(paths.table(table.name))}>
      <Card style={{ transition: 'transform 120ms' }} className="forge-lift">
        <Group wrap="nowrap">
          <ThemeIcon size={48} radius="md" variant="gradient" gradient={{ ...gradient, deg: 135 }}>
            <Icon size={26} />
          </ThemeIcon>
          <div>
            <Text size="sm" c="dimmed" fw={600}>
              {forge.format.tableLabel(table)}
            </Text>
            <Text fz={28} fw={800} lh={1.1}>
              {count.data ? forge.format.number(count.data.total) : '–'}
            </Text>
          </div>
        </Group>
      </Card>
    </UnstyledButton>
  );
}

/** Répartition de la première mesure de la vue `stats`. */
function StatsCard({ table }: { table: TableSchema }) {
  const forge = useForge();
  const view = table.stats!;
  const field = findColumn(table, view.fields[0]);
  const groupBy = view.group_by ? findColumn(table, view.group_by) : undefined;
  const data = useAsync(
    () => forge.client.table(table.name).aggregate(view.fields.slice(0, 1), view.group_by),
    [table.name],
  );
  if (!field || !groupBy) return null;
  if (!data.data) return <Loading />;
  return (
    <GroupBars
      title={`${forge.format.tableLabel(table)} · ${forge.format.columnLabel(field)} (${forge.strings.sum})`}
      table={table}
      groupBy={groupBy}
      groups={data.data.groups}
      value={(g) => g.measures[field.name]?.sum ?? 0}
      format={(v) => formatMeasure(forge, field, v)}
    />
  );
}

/** Prochaines échéances d'une table à calendrier (30 jours). */
function UpcomingCard({ table }: { table: TableSchema }) {
  const forge = useForge();
  const navigate = useNavigate();
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const end = new Date(today.getFullYear(), today.getMonth(), today.getDate() + 30);
  const events = useAsync(() => loadEvents(forge.client, table, {}, today, end), [table.name]);
  const startColumn = findColumn(table, table.calendar!.start);
  return (
    <Card>
      <Title order={5} mb="sm">
        {forge.format.tableLabel(table)} · {forge.format.columnLabel(startColumn!)}
      </Title>
      {!events.data ? (
        <Loading />
      ) : events.data.length === 0 ? (
        <Text c="dimmed" size="sm">
          {forge.strings.noResults}
        </Text>
      ) : (
        <Stack gap="xs">
          {events.data.slice(0, 6).map((e) => (
            <UnstyledButton key={e.record.id as number} onClick={() => navigate(paths.record(table.name, e.record.id as number))}>
              <Group wrap="nowrap" gap="sm">
                <ThemeIcon variant="light" radius="md">
                  <IconCalendarEvent size={16} />
                </ThemeIcon>
                <div style={{ minWidth: 0 }}>
                  <Text size="sm" fw={600} truncate>
                    {forge.format.title(table, e.record)}
                  </Text>
                  <Text size="xs" c="dimmed">
                    {e.allDay ? forge.format.date(e.start) : forge.format.dateTime(e.start)}
                  </Text>
                </div>
              </Group>
            </UnstyledButton>
          ))}
        </Stack>
      )}
    </Card>
  );
}

/** Accueil : bienvenue, indicateurs par table, répartitions et échéances. */
export function HomePage() {
  const forge = useForge();
  const readable = forge.schema.tables.filter((t) => can(forge, t, 'read'));
  const now = new Date();
  return (
    <Stack gap="lg">
      <Paper
        p="xl"
        radius="lg"
        style={{
          color: 'white',
          background:
            'linear-gradient(120deg, var(--mantine-color-indigo-7), var(--mantine-color-violet-6) 60%, var(--mantine-color-pink-5))',
        }}
      >
        <Title order={2}>
          {forge.strings.welcome}
          {forge.user ? `, ${userName(forge.user)}` : ''}
        </Title>
        <Text opacity={0.85} tt="capitalize">
          {new Intl.DateTimeFormat(forge.locale.replace('_', '-'), { dateStyle: 'full' }).format(now)}
        </Text>
      </Paper>
      <SimpleGrid cols={{ base: 1, xs: 2, lg: 4 }}>
        {readable.map((t, i) => (
          <TableTile key={t.name} table={t} index={i} />
        ))}
      </SimpleGrid>
      <SimpleGrid cols={{ base: 1, lg: 2 }}>
        {readable.filter((t) => t.stats?.group_by).map((t) => (
          <StatsCard key={t.name} table={t} />
        ))}
        {readable.filter((t) => t.calendar).map((t) => (
          <UpcomingCard key={t.name} table={t} />
        ))}
      </SimpleGrid>
    </Stack>
  );
}
